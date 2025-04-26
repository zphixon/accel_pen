use crate::error::{ApiError, ApiErrorInner, Context};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_sessions::Session;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NadeoOauth {
    token_type: String,
    pub expires_in: u64,
    pub access_token: String,
    pub refresh_token: String,
}

pub struct AuthenticatedSession {
    session: Session,
    tokens: Arc<NadeoOauth>,
}

impl AuthenticatedSession {
    const KEY: &str = "authSession";

    pub fn tokens(&self) -> &NadeoOauth {
        &self.tokens
    }

    pub async fn swap_tokens(&mut self, tokens: &NadeoOauth) -> Result<(), ApiError> {
        self.tokens = Arc::new(tokens.clone());
        Self::update_session(&self.session, &self.tokens).await
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub async fn update_session(session: &Session, tokens: &NadeoOauth) -> Result<(), ApiError> {
        session
            .insert(Self::KEY, tokens.clone())
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
            .get::<NadeoOauth>(AuthenticatedSession::KEY)
            .await
            .context("Reading auth from session")?
        else {
            return Err(ApiErrorInner::Rejected((
                StatusCode::UNAUTHORIZED,
                "Not authenticated by Nadeo",
            ))
            .into());
        };

        Ok(Self {
            session,
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
