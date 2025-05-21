use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::{error::Error, fmt::Display, ops::Deref};
use ts_rs::TS;

#[derive(Debug, Serialize, thiserror::Error, strum::IntoStaticStr, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum ApiErrorInner {
    #[error("DB error: {error}")]
    Database {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: migration::DbErr,
    },

    #[error("Invalid query: {error}")]
    InvalidQuery {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::extract::rejection::QueryRejection,
    },

    #[error("URL parse error: {error}")]
    UrlParseError {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: url::ParseError,
    },

    #[error("Session error: {error}")]
    SessionError {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: tower_sessions::session::Error,
    },

    #[error("Axum error: {error}")]
    AxumError {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::http::Error,
    },

    #[error("Request to API failed: {error}")]
    ApiFailed {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: reqwest::Error,
    },

    #[error("Request to API failed: {error}")]
    ApiReturnedError {
        #[ts(skip)]
        error: serde_json::Value,
    },

    #[error("Invalid JSON: {error}")]
    InvalidJson {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::extract::rejection::JsonRejection,
    },

    #[error("Invalid path: {error}")]
    InvalidPath {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::extract::rejection::PathRejection,
    },

    #[error("Invalid oauth: {error}")]
    InvalidOauth {
        #[ts(skip)]
        error: &'static str,
    },

    #[error("Unexpected response from Nadeo API: {error}")]
    UnexpectedResponse {
        #[ts(skip)]
        error: &'static str,
    },

    #[error("Rejected: {}", error.1)]
    Rejected {
        #[ts(skip)]
        #[serde(skip)]
        error: (axum::http::StatusCode, &'static str),
    },

    #[error("Map not found: {map_id}")]
    MapNotFound {
        #[ts(skip)]
        map_id: i32,
    },

    #[error("Multipart error: {error}")]
    MultipartError {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::extract::multipart::MultipartError,
    },

    #[error("Invalid multipart: {error}")]
    InvalidMultipart {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: axum::extract::multipart::MultipartRejection,
    },

    #[error("Missing from multipart field: {error}")]
    MissingFromMultipart {
        #[ts(skip)]
        error: &'static str,
    },

    #[error("Map has not been validated")]
    NotValidated,

    #[error("Invalid GBX data: {error}")]
    InvalidGbx {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: gbx_rs::GbxError,
    },

    #[error("Not a map")]
    NotAMap,

    #[error("Map already uploaded")]
    AlreadyUploaded { map_id: i32 },

    #[error("This map was created by a user already on Accel Pen")]
    NotYourMap,

    #[error("Invalid map thumbnail: {error}")]
    InvalidThumbnail {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: image::error::ImageError,
    },

    #[error("Standard library IO error: {error}")]
    StdIo {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: std::io::Error,
    },

    #[error("No such tag: {tag}")]
    NoSuchTag { tag: String },

    #[error("Too many tags, max {max}")]
    TooManyTags { max: i32 },

    #[error("Not base64")]
    NotBase64 {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: base64::DecodeError,
    },

    #[error("Invalid UTF-8")]
    NotUtf8 {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: std::str::Utf8Error,
    },

    #[error("Not UUID")]
    NotUuid {
        #[serde(skip)]
        #[ts(skip)]
        #[from]
        error: uuid::Error,
    },

    #[error("No such API: {error}")]
    NotFound {
        #[ts(skip)]
        error: String,
    },

    #[error("Could not parse time value: {error}")]
    Time {
        #[ts(skip)]
        #[serde(skip)]
        #[from]
        error: time::error::Format,
    },

    #[error("Could not parse templates: {error} {source:?}", source = .error.source())]
    Tera {
        #[ts(skip)]
        #[serde(skip)]
        #[from]
        error: tera::Error,
    },

    #[error("Could not parse JSON: {error}")]
    Json {
        #[ts(skip)]
        #[serde(skip)]
        #[from]
        error: serde_json::Error,
    },
}

impl From<(axum::http::StatusCode, &'static str)> for ApiErrorInner {
    fn from(value: (axum::http::StatusCode, &'static str)) -> Self {
        ApiErrorInner::Rejected { error: value }
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
    error: ApiErrorInner,
    status: u16,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!("{}", self);

        let status_code = match &*self {
            ApiErrorInner::Database { .. }
            | ApiErrorInner::UrlParseError { .. }
            | ApiErrorInner::SessionError { .. }
            | ApiErrorInner::AxumError { .. }
            | ApiErrorInner::ApiReturnedError { .. }
            | ApiErrorInner::UnexpectedResponse { .. }
            | ApiErrorInner::Tera { .. }
            | ApiErrorInner::StdIo { .. }
            | ApiErrorInner::Time { .. } => StatusCode::INTERNAL_SERVER_ERROR,

            ApiErrorInner::ApiFailed { error } => error.status().unwrap_or(StatusCode::BAD_GATEWAY),

            ApiErrorInner::InvalidQuery { .. }
            | ApiErrorInner::InvalidPath { .. }
            | ApiErrorInner::InvalidJson { .. }
            | ApiErrorInner::InvalidMultipart { .. }
            | ApiErrorInner::MissingFromMultipart { .. }
            | ApiErrorInner::InvalidGbx { .. }
            | ApiErrorInner::MultipartError { .. }
            | ApiErrorInner::NotAMap
            | ApiErrorInner::NotBase64 { .. }
            | ApiErrorInner::NotUtf8 { .. }
            | ApiErrorInner::NotYourMap
            | ApiErrorInner::AlreadyUploaded { .. }
            | ApiErrorInner::Json { .. }
            | ApiErrorInner::NoSuchTag { .. }
            | ApiErrorInner::TooManyTags { .. }
            | ApiErrorInner::InvalidThumbnail { .. }
            | ApiErrorInner::NotValidated
            | ApiErrorInner::NotUuid { .. } => StatusCode::BAD_REQUEST,

            ApiErrorInner::InvalidOauth { .. } => StatusCode::UNAUTHORIZED,

            ApiErrorInner::MapNotFound { .. } | ApiErrorInner::NotFound { error: _ } => {
                StatusCode::NOT_FOUND
            }

            ApiErrorInner::Rejected { error: (code, _) } => *code,
        };

        let message = self.to_string();
        match serde_json::to_string(&TsApiError {
            error: self.into_inner(),
            status: status_code.as_u16(),
            message,
        }) {
            Ok(json) => (status_code, json).into_response(),
            Err(serde_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Couldn't format json??? {}", serde_err),
            )
                .into_response(),
        }
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

impl ApiError {
    pub fn into_inner(self) -> ApiErrorInner {
        match self {
            ApiError::Root(api_error_inner) => api_error_inner,
            ApiError::Context { inner, .. } => inner.into_inner(),
        }
    }

    //pub fn inner(&self) -> &ApiErrorInner {
    //    match self {
    //        ApiError::Root(api_error_inner) => api_error_inner,
    //        ApiError::Context { inner, .. } => inner.inner(),
    //    }
    //}
}
