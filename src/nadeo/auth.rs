use crate::{
    config::CONFIG,
    error::{ApiError, ApiErrorInner, Context},
    nadeo::api::{NadeoClubTag, NadeoUser, CLIENT},
    AppState,
};
use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::{request::Parts, StatusCode},
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
};
use tower_sessions::Session;
use url::Url;
use uuid::Uuid;

static NADEO_OAUTH_CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    let Ok(secret) = std::fs::read_to_string(&CONFIG.nadeo.oauth.secret_path) else {
        panic!("Couldn't read nadeo client secret file");
    };
    secret.trim().to_owned()
});

const NADEO_OAUTH_AUTHORIZE_URL: &str = "https://api.trackmania.com/oauth/authorize";
const NADEO_OAUTH_GET_ACCESS_TOKEN_URL: &str = "https://api.trackmania.com/api/access_token";

pub fn oauth_start_url(state: Uuid) -> Result<Url, ApiError> {
    Ok(Url::parse_with_params(
        NADEO_OAUTH_AUTHORIZE_URL,
        &[
            ("response_type", "code"),
            ("client_id", &CONFIG.nadeo.oauth.identifier),
            ("scope", "read_favorite write_favorite"),
            ("redirect_uri", CONFIG.oauth_redirect_url().as_str()),
            ("state", state.as_hyphenated().to_string().as_str()),
        ],
    )
    .context("Creating redirect URL to Nadeo")?)
}

#[derive(Deserialize)]
pub struct NadeoOauthFinishRequest {
    code: String,
    state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NadeoTokensInner {
    token_type: String,
    expires_in: u32,
    access_token: String,
    refresh_token: String,
}

impl NadeoTokensInner {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

/// OAuth-authenticated access tokens
#[derive(Clone, Serialize, Deserialize)]
pub struct NadeoAuthSessionInner {
    inner: NadeoTokensInner,
    user: NadeoUser,
    club_tag: Option<String>,
    user_id: i32,
    issued: time::OffsetDateTime,
    registered: time::OffsetDateTime,
}

impl Deref for NadeoAuthSessionInner {
    type Target = NadeoTokensInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl NadeoAuthSessionInner {
    async fn from_inner(inner: NadeoTokensInner, state: &AppState) -> Result<Self, ApiError> {
        let issued = time::OffsetDateTime::now_utc();
        let user = NadeoUser::get_self(&inner).await?;
        let club_tag = NadeoClubTag::get(&user.account_id)
            .await
            .context("Get self club tag")?;

        let registered = time::OffsetDateTime::now_utc();

        let mut conn = state.db.get().await?;
        let new_user: crate::models::User = diesel::insert_into(crate::schema::ap_user::table)
            .values(crate::models::NewUser {
                nadeo_display_name: user.display_name.clone(),
                nadeo_login: crate::nadeo::account_id_to_login(&user.account_id)?,
                nadeo_account_id: user.account_id.clone(),
                nadeo_club_tag: club_tag.clone(),
                registered: Some(registered.clone()),
            })
            .on_conflict(crate::schema::ap_user::dsl::nadeo_account_id)
            .do_update()
            .set((
                crate::schema::ap_user::dsl::nadeo_display_name.eq(user.display_name.clone()),
                crate::schema::ap_user::dsl::nadeo_club_tag.eq(club_tag),
                crate::schema::ap_user::dsl::registered.eq(Some(registered)),
            ))
            .returning(crate::models::User::as_returning())
            .get_result(&mut conn)
            .await?;

        Ok(NadeoAuthSessionInner {
            inner,
            user,
            club_tag: new_user.nadeo_club_tag,
            user_id: new_user.ap_user_id,
            issued,
            registered: new_user.registered.expect("registered"),
        })
    }

    pub fn account_id(&self) -> &str {
        &self.user.account_id
    }

    pub fn display_name(&self) -> &str {
        &self.user.display_name
    }

    pub fn club_tag(&self) -> Option<&str> {
        self.club_tag.as_deref()
    }

    pub fn user_id(&self) -> i32 {
        self.user_id
    }

    pub fn registered(&self) -> time::OffsetDateTime {
        self.registered
    }

    //pub fn oauth_access_token(&self) -> &str {
    //    &self.inner.access_token
    //}

    pub fn expired(&self) -> bool {
        let margin = time::Duration::seconds(self.inner.expires_in.saturating_sub(30) as i64);
        let expiry = self.issued + margin;
        time::OffsetDateTime::now_utc() > expiry
    }

    async fn from_random_state_session(
        state: &AppState,
        random_state: &RandomStateSession,
        request: NadeoOauthFinishRequest,
    ) -> Result<Self, ApiError> {
        if random_state.state().hyphenated().to_string() != request.state {
            return Err(ApiErrorInner::InvalidOauth {
                error: "Invalid random state returned from Nadeo API",
            }
            .into());
        }

        let params = form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
            .append_pair("client_secret", &NADEO_OAUTH_CLIENT_SECRET)
            .append_pair("code", &request.code)
            .append_pair("redirect_uri", CONFIG.oauth_redirect_url().as_str())
            .finish();

        let response = CLIENT
            .clone()
            .post(Url::parse(NADEO_OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(params)
            .send()
            .await
            .context("Sending request for access token")?;

        if response.status().is_success() {
            let nadeo_oauth: NadeoTokensInner = response
                .json()
                .await
                .context("Parsing oauth tokens from Nadeo")?;
            Self::from_inner(nadeo_oauth, state).await
        } else {
            let json_error: serde_json::Value = response.json().await?;
            Err(ApiErrorInner::ApiReturnedError { error: json_error }.into())
        }
    }

    async fn refresh(self, state: &AppState) -> Result<Self, ApiError> {
        let params = form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "refresh_token")
            .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
            .append_pair("client_secret", &NADEO_OAUTH_CLIENT_SECRET)
            .append_pair("refresh_token", &self.inner.refresh_token)
            .finish();

        let response = CLIENT
            .clone()
            .post(Url::parse(NADEO_OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(params)
            .send()
            .await
            .context("Sending request for refresh token")?;

        if response.status().is_success() {
            let inner: NadeoTokensInner = response
                .json()
                .await
                .context("Parsing oauth tokens from Nadeo")?;
            NadeoAuthSessionInner::from_inner(inner, state).await
        } else {
            let json_error: serde_json::Value = response.json().await?;
            Err(ApiErrorInner::ApiReturnedError { error: json_error }.into())
        }
    }
}

pub struct NadeoAuthSession {
    random_state_session: RandomStateSession,
    inner: Arc<NadeoAuthSessionInner>,
}

impl Deref for NadeoAuthSession {
    type Target = NadeoAuthSessionInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl NadeoAuthSession {
    const KEY: &str = "authSession";

    pub async fn upgrade(
        state: &AppState,
        random_state_session: RandomStateSession,
        request: NadeoOauthFinishRequest,
    ) -> Result<NadeoAuthSession, ApiError> {
        let inner = NadeoAuthSessionInner::from_random_state_session(
            &state,
            &random_state_session,
            request,
        )
        .await?;

        Self::upgrade_with(random_state_session, inner).await
    }

    async fn upgrade_with(
        random_state_session: RandomStateSession,
        inner: NadeoAuthSessionInner,
    ) -> Result<Self, ApiError> {
        random_state_session
            .session
            .insert(Self::KEY, inner.clone())
            .await
            .context("Writing tokens to session")?;

        Ok(NadeoAuthSession {
            random_state_session,
            inner: Arc::new(inner),
        })
    }

    pub fn session(&self) -> &Session {
        &self.random_state_session.session
    }

    pub fn return_path(&self) -> Option<&str> {
        self.random_state_session.return_path()
    }
}

impl FromRequestParts<AppState> for NadeoAuthSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let random_state_session = RandomStateSession::from_request_parts(parts, state).await?;

        let Some(tokens) = random_state_session
            .session
            .get::<NadeoAuthSessionInner>(NadeoAuthSession::KEY)
            .await
            .context("Reading auth from session")?
        else {
            return Err(ApiErrorInner::Rejected {
                error: (StatusCode::UNAUTHORIZED, "Not authenticated by Nadeo"),
            }
            .into());
        };

        if tokens.expired() {
            tracing::debug!("access token about to expire, refreshing");

            let tokens = tokens
                .refresh(state)
                .await
                .context("Refreshing token while extracting authenticated session")?;

            let session = NadeoAuthSession::upgrade_with(random_state_session, tokens)
                .await
                .context("Setting session after refreshing")?;

            tracing::debug!("successfully refreshed");

            Ok(session)
        } else {
            NadeoAuthSession::upgrade_with(random_state_session, tokens).await
        }
    }
}

impl OptionalFromRequestParts<AppState> for NadeoAuthSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequestParts<AppState>>::from_request_parts(parts, state).await {
            Ok(auth) => Ok(Some(auth)),
            Err(_) => Ok(None),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RandomStateSessionInner {
    state: Uuid,
    return_path: Option<String>,
}

pub struct RandomStateSession {
    session: Session,
    inner: RandomStateSessionInner,
}

impl RandomStateSession {
    const KEY: &str = "randomState";

    pub fn state(&self) -> &Uuid {
        &self.inner.state
    }

    pub fn return_path(&self) -> Option<&str> {
        self.inner.return_path.as_deref()
    }

    pub async fn update_session(
        session: &Session,
        state: Uuid,
        return_path: Option<String>,
    ) -> Result<(), ApiError> {
        session
            .insert(Self::KEY, RandomStateSessionInner { state, return_path })
            .await
            .context("Writing state to session")?;
        Ok(())
    }
}

impl FromRequestParts<AppState> for RandomStateSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;

        let Some(data) = session
            .get::<RandomStateSessionInner>(RandomStateSession::KEY)
            .await
            .context("Reading state from session")?
        else {
            return Err(ApiErrorInner::Rejected {
                error: (StatusCode::UNAUTHORIZED, "No oauth flow in progress"),
            }
            .into());
        };

        Ok(Self {
            session,
            inner: data,
        })
    }
}
