use crate::{
    api::{User, CLIENT},
    config::CONFIG,
    error::{ApiError, ApiErrorInner, Context},
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
};
use tower_sessions::Session;
use url::Url;
use uuid::Uuid;

static OAUTH_CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    let Ok(secret) = std::fs::read_to_string(&CONFIG.nadeo.oauth.secret_path) else {
        panic!("Couldn't read nadeo client secret file");
    };
    secret.trim().to_owned()
});

const OAUTH_AUTHORIZE_URL: &str = "https://api.trackmania.com/oauth/authorize";
const OAUTH_GET_ACCESS_TOKEN_URL: &str = "https://api.trackmania.com/api/access_token";

pub fn oauth_start_url(state: Uuid) -> Result<Url, ApiError> {
    Ok(Url::parse_with_params(
        OAUTH_AUTHORIZE_URL,
        &[
            ("response_type", "code"),
            ("client_id", &CONFIG.nadeo.oauth.identifier),
            ("scope", "read_favorite write_favorite"),
            ("redirect_uri", CONFIG.nadeo.oauth.redirect_url.as_str()),
            ("state", state.as_hyphenated().to_string().as_str()),
        ],
    )
    .context("Creating redirect URL to Nadeo")?)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NadeoTokenPairInner {
    token_type: String,
    expires_in: u32,
    access_token: String,
    refresh_token: String,
}

impl NadeoTokenPairInner {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NadeoTokenPair {
    inner: NadeoTokenPairInner,
    user: User,
    issued: time::OffsetDateTime,
}

impl NadeoTokenPair {
    async fn from_nadeo(token_pair: NadeoTokenPairInner) -> Result<Self, ApiError> {
        let issued = time::OffsetDateTime::now_utc();
        let user = User::get(&token_pair).await?;

        Ok(NadeoTokenPair {
            inner: token_pair,
            user,
            issued,
        })
    }

    pub fn account_id(&self) -> &str {
        &self.user.account_id
    }

    pub fn display_name(&self) -> &str {
        &self.user.display_name
    }

    fn refresh_token(&self) -> &str {
        &self.inner.refresh_token
    }

    pub fn access_token(&self) -> &str {
        &self.inner.access_token
    }

    pub fn expired(&self) -> bool {
        let margin = time::Duration::seconds(self.inner.expires_in.saturating_sub(30) as i64);
        let expiry = self.issued + margin;
        time::OffsetDateTime::now_utc() > expiry
    }

    pub async fn from_random_state(
        random_state: &RandomStateSession,
        request: crate::OauthFinishRequest,
    ) -> Result<Self, ApiError> {
        if random_state.state().hyphenated().to_string() != request.state {
            return Err(ApiErrorInner::OauthFailed(String::from(
                "Invalid random state returned from Nadeo API",
            ))
            .into());
        }

        let params = form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
            .append_pair("client_secret", &OAUTH_CLIENT_SECRET)
            .append_pair("code", &request.code)
            .append_pair("redirect_uri", CONFIG.nadeo.oauth.redirect_url.as_str())
            .finish();

        let response = CLIENT
            .clone()
            .post(Url::parse(OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(params)
            .send()
            .await
            .context("Sending request for access token")?;

        if response.status().is_success() {
            let nadeo_oauth: NadeoTokenPairInner = response
                .json()
                .await
                .context("Parsing oauth tokens from Nadeo")?;
            Self::from_nadeo(nadeo_oauth).await
        } else {
            let json_error: serde_json::Value = response.json().await?;
            Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
        }
    }

    async fn refresh(self) -> Result<Self, ApiError> {
        let params = form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "refresh_token")
            .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
            .append_pair("client_secret", &OAUTH_CLIENT_SECRET)
            .append_pair("refresh_token", &self.inner.refresh_token)
            .finish();

        let response = CLIENT
            .clone()
            .post(Url::parse(OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(params)
            .send()
            .await
            .context("Sending request for refresh token")?;

        if response.status().is_success() {
            let token_pair: NadeoTokenPairInner = response
                .json()
                .await
                .context("Parsing oauth tokens from Nadeo")?;
            NadeoTokenPair::from_nadeo(token_pair).await
        } else {
            let json_error: serde_json::Value = response.json().await?;
            Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
        }
    }
}

pub struct NadeoAuthenticatedSession {
    tokens: Arc<NadeoTokenPair>,
}

impl Deref for NadeoAuthenticatedSession {
    type Target = NadeoTokenPair;
    fn deref(&self) -> &Self::Target {
        &self.tokens
    }
}

impl NadeoAuthenticatedSession {
    const KEY: &str = "authSession";

    pub async fn upgrade(
        session: &RandomStateSession,
        tokens: NadeoTokenPair,
    ) -> Result<(), ApiError> {
        session
            .session
            .insert(Self::KEY, tokens)
            .await
            .context("Writing tokens to session")?;
        Ok(())
    }
}

impl<S> FromRequestParts<S> for NadeoAuthenticatedSession
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = RandomStateSession::from_request_parts(parts, state).await?;

        let Some(tokens) = session
            .session
            .get::<NadeoTokenPair>(NadeoAuthenticatedSession::KEY)
            .await
            .context("Reading auth from session")?
        else {
            return Err(ApiErrorInner::Rejected((
                StatusCode::UNAUTHORIZED,
                "Not authenticated by Nadeo",
            ))
            .into());
        };

        let tokens = if tokens.expired() {
            tracing::debug!("access token about to expire, refreshing");

            let tokens = tokens
                .refresh()
                .await
                .context("Refreshing token while extracting authenticated session")?;

            NadeoAuthenticatedSession::upgrade(&session, tokens.clone())
                .await
                .context("Setting session after refreshing")?;

            tracing::debug!("successfully refreshed");

            tokens
        } else {
            tokens
        };

        Ok(Self {
            tokens: Arc::new(tokens),
        })
    }
}

pub struct RandomStateSession {
    session: Session,
    state: Uuid,
}

impl RandomStateSession {
    const KEY: &str = "randomState";

    pub fn state(&self) -> &Uuid {
        &self.state
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub async fn update_session(session: &Session, state: Uuid) -> Result<(), ApiError> {
        session
            .insert(Self::KEY, state)
            .await
            .context("Writing state to session")?;
        Ok(())
    }
}

impl<S> FromRequestParts<S> for RandomStateSession
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;

        let Some(state) = session
            .get::<Uuid>(RandomStateSession::KEY)
            .await
            .context("Reading state from session")?
        else {
            return Err(ApiErrorInner::Rejected((
                StatusCode::UNAUTHORIZED,
                "No oauth flow in progress",
            ))
            .into());
        };

        Ok(Self { session, state })
    }
}
