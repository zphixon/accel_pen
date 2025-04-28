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

    #[error("Invalid query: {0}")]
    InvalidQuery(
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

    #[error("Request to API failed: {0}")]
    ApiFailed(
        #[ts(skip)]
        #[from]
        reqwest::Error,
    ),

    #[error("Request to API failed: {0}")]
    ApiReturnedError(#[ts(skip)] serde_json::Value),

    #[error("Invalid JSON: {0}")]
    InvalidJson(
        #[ts(skip)]
        #[from]
        axum::extract::rejection::JsonRejection,
    ),

    #[error("Invalid oauth: {0}")]
    InvalidOauth(#[ts(skip)] &'static str),

    #[error("Unexpected response from Nadeo API: {0}")]
    UnexpectedResponse(#[ts(skip)] &'static str),

    #[error("Rejected: {}", .0.1)]
    Rejected(#[ts(skip)] (axum::http::StatusCode, &'static str)),

    #[error("Map not found: {0}")]
    MapNotFound(#[ts(skip)] u32),

    #[error("Multipart error: {0}")]
    MultipartError(
        #[ts(skip)]
        #[from]
        axum::extract::multipart::MultipartError,
    ),

    #[error("Invalid multipart: {0}")]
    InvalidMultipart(
        #[ts(skip)]
        #[from]
        axum::extract::multipart::MultipartRejection,
    ),

    #[error("Missing from multipart field: {0}")]
    MissingFromMultipart(#[ts(skip)] &'static str),

    #[error("Invalid GBX data: {0}")]
    InvalidGbx(
        #[ts(skip)]
        #[from]
        gbx_rs::GbxError,
    ),

    #[error("Not a map")]
    NotAMap,

    #[error("Cannot upload map that isn't yours")]
    NotYourMap,

    #[error("Not base64")]
    NotBase64(
        #[ts(skip)]
        #[from]
        base64::DecodeError,
    ),

    #[error("Invalid UTF-8")]
    NotUtf8(#[ts(skip)] #[from] std::str::Utf8Error),

    #[error("Not UUID")]
    NotUuid(
        #[ts(skip)]
        #[from]
        uuid::Error,
    ),

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
    status: u16,
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
            | ApiErrorInner::AxumError(_)
            | ApiErrorInner::ApiReturnedError(_)
            | ApiErrorInner::UnexpectedResponse(_) => StatusCode::INTERNAL_SERVER_ERROR,

            ApiErrorInner::ApiFailed(err) => err.status().unwrap_or(StatusCode::BAD_GATEWAY),

            ApiErrorInner::InvalidQuery(_)
            | ApiErrorInner::InvalidJson(_)
            | ApiErrorInner::InvalidMultipart(_)
            | ApiErrorInner::MissingFromMultipart(_)
            | ApiErrorInner::InvalidGbx(_)
            | ApiErrorInner::MultipartError(_)
            | ApiErrorInner::NotAMap
            | ApiErrorInner::NotBase64(_)
            | ApiErrorInner::NotUtf8(_)
            | ApiErrorInner::NotYourMap
            | ApiErrorInner::NotUuid(_) => StatusCode::BAD_REQUEST,

            ApiErrorInner::InvalidOauth(_) => StatusCode::UNAUTHORIZED,

            ApiErrorInner::MapNotFound(_) | ApiErrorInner::NotFound(_) => StatusCode::NOT_FOUND,

            ApiErrorInner::Rejected((code, _)) => *code,
        };

        (
            status_code,
            Json(TsApiError {
                error: error.to_owned(),
                status: status_code.as_u16(),
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
