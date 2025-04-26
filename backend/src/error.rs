use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::{fmt::Display, ops::Deref};
use ts_rs::TS;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr, TS)]
#[ts(export)]
pub enum ApiErrorInner {
    #[error("Database error: {0}")]
    Database(
        #[ts(skip)]
        #[from]
        sqlx::Error,
    ),

    #[error("Migration error: {0}")]
    Migration(
        #[ts(skip)]
        #[from]
        sqlx::migrate::MigrateError,
    ),

    #[error("Invalid map data request: {0}")]
    InvalidMapDataRequest(
        #[ts(skip)]
        #[from]
        axum::extract::rejection::QueryRejection,
    ),

    #[error("URL parse error: {0}")]
    UrlParseError(
        #[ts(skip)]
        #[from]
        url::ParseError,
    ),

    #[error("Session error: {0}")]
    SessionError(
        #[ts(skip)]
        #[from]
        tower_sessions::session::Error,
    ),

    #[error("Axum error: {0}")]
    AxumError(
        #[ts(skip)]
        #[from]
        axum::http::Error,
    ),

    #[error("Request to Nadeo API failed: {0}")]
    NadeoApiFailed(
        #[ts(skip)]
        #[from]
        reqwest::Error,
    ),

    #[error("Oauth failed: {0}")]
    OauthFailed(#[ts(skip)] String),

    #[error("Invalid OAuth request: {0}")]
    InvalidOauth(
        #[ts(skip)]
        #[from]
        axum::extract::rejection::JsonRejection,
    ),

    #[error("Rejected: {}", .0.1)]
    Rejected(#[ts(skip)] (axum::http::StatusCode, &'static str)),

    #[error("Map not found: {0}")]
    MapNotFound(#[ts(skip)] u32),

    #[error("No such API: {0}")]
    NotFound(#[ts(skip)] String),
}

impl From<(axum::http::StatusCode, &'static str)> for ApiErrorInner {
    fn from(value: (axum::http::StatusCode, &'static str)) -> Self {
        ApiErrorInner::Rejected(value)
    }
}

#[derive(Debug)]
pub enum ApiError {
    Root(ApiErrorInner),
    Context {
        context: String,
        inner: Box<ApiError>,
    },
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct TsApiError {
    #[ts(as = "ApiErrorInner")]
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!("{}", self);

        let error: &'static str = (&*self).into();
        let status_code = match &*self {
            ApiErrorInner::Database(_)
            | ApiErrorInner::Migration(_)
            | ApiErrorInner::UrlParseError(_)
            | ApiErrorInner::SessionError(_)
            | ApiErrorInner::AxumError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorInner::NadeoApiFailed(err) => err.status().unwrap_or(StatusCode::BAD_GATEWAY),
            ApiErrorInner::InvalidMapDataRequest(_)
            | ApiErrorInner::OauthFailed(_)
            | ApiErrorInner::InvalidOauth(_) => StatusCode::BAD_REQUEST,
            ApiErrorInner::MapNotFound(_) | ApiErrorInner::NotFound(_) => StatusCode::NOT_FOUND,
            ApiErrorInner::Rejected((code, _)) => *code,
        };

        (
            status_code,
            Json(TsApiError {
                error: error.to_owned(),
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let inner: &ApiErrorInner = &*self;
        Some(inner)
    }
}

pub trait Context<T> {
    fn context<C>(self, context: C) -> Result<T, ApiError>
    where
        C: std::fmt::Display + Send + Sync + 'static;

    fn with_context<F, C>(self, context_fn: F) -> Result<T, ApiError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display + Send + Sync + 'static;
}

impl<T, E: Into<ApiError>> Context<T> for Result<T, E> {
    fn context<C>(self, context: C) -> Result<T, ApiError>
    where
        C: std::fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(err) => Err(ApiError::Context {
                context: context.to_string(),
                inner: Box::new(err.into()),
            }),
        }
    }

    fn with_context<F, C>(self, context_fn: F) -> Result<T, ApiError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(err) => Err(ApiError::Context {
                context: context_fn().to_string(),
                inner: Box::new(err.into()),
            }),
        }
    }
}

impl Deref for ApiError {
    type Target = ApiErrorInner;

    fn deref(&self) -> &Self::Target {
        match self {
            ApiError::Root(inner) => inner,
            ApiError::Context { inner, .. } => {
                let box_ref = Box::as_ref(inner);
                <ApiError as Deref>::deref(box_ref)
            }
        }
    }
}

impl<T: Into<ApiErrorInner>> From<T> for ApiError {
    fn from(value: T) -> Self {
        ApiError::Root(value.into())
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Root(inner) => write!(f, "{}", inner),
            ApiError::Context { context, inner } => {
                Display::fmt(inner, f)?;
                write!(f, "\n  {}", context)
            }
        }
    }
}
