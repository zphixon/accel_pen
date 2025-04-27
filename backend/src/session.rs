use crate::{
    auth::nadeo::{self, NadeoTokenPair}, error::{ApiError, ApiErrorInner, Context},
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use std::sync::Arc;
use tower_sessions::Session;
use uuid::Uuid;

pub struct NadeoAuthenticatedSession {
    tokens: Arc<NadeoTokenPair>,
}

impl NadeoAuthenticatedSession {
    const KEY: &str = "authSession";

    pub fn tokens(&self) -> &NadeoTokenPair {
        &self.tokens
    }

    pub async fn update_session(session: &Session, tokens: NadeoTokenPair) -> Result<(), ApiError> {
        session
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
        let session = Session::from_request_parts(parts, state).await?;

        let Some(tokens) = session
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

            let tokens = nadeo::refresh(tokens)
                .await
                .context("Refreshing token while extracting authenticated session")?;

            NadeoAuthenticatedSession::update_session(&session, tokens.clone())
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
