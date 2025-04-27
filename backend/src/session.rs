use crate::{
    error::{ApiError, ApiErrorInner, Context},
    nadeo,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_sessions::Session;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NadeoTokenPair {
    token_type: String,
    expires_in: u32,
    access_token: String,
    refresh_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TokenPair {
    inner: NadeoTokenPair,
    issued: time::OffsetDateTime,
}

impl TokenPair {
    pub fn from_nadeo(nadeo_token_pair: NadeoTokenPair) -> Self {
        TokenPair {
            inner: nadeo_token_pair,
            issued: time::OffsetDateTime::now_utc(),
        }
    }

    pub fn refresh_token(&self) -> &str {
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
}

pub struct AuthenticatedSession {
    tokens: Arc<TokenPair>,
}

impl AuthenticatedSession {
    const KEY: &str = "authSession";

    pub fn tokens(&self) -> &TokenPair {
        &self.tokens
    }

    pub async fn update_session(session: &Session, tokens: TokenPair) -> Result<(), ApiError> {
        session
            .insert(Self::KEY, tokens)
            .await
            .context("Writing tokens to session")?;
        Ok(())
    }
}

impl<S> FromRequestParts<S> for AuthenticatedSession
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;

        let Some(tokens) = session
            .get::<TokenPair>(AuthenticatedSession::KEY)
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

            let tokens = nadeo::refresh(tokens)
                .await
                .context("Refreshing token while extracting authenticated session")?;

            AuthenticatedSession::update_session(&session, tokens.clone())
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

pub struct RandomState {
    session: Session,
    state: Uuid,
}

impl RandomState {
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

impl<S> FromRequestParts<S> for RandomState
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;

        let Some(state) = session
            .get::<Uuid>(RandomState::KEY)
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
