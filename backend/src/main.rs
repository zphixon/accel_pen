use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::{fmt::Display, net::SocketAddr, ops::Deref, path::PathBuf, sync::LazyLock};
use strum::VariantNames;
use tokio::net::TcpListener;
use tower_http::cors::{self, CorsLayer};
use ts_rs::TS;
use url::Url;

from_env::config!(
    "ACCEL_PEN",
    net {
        #[serde(default)]
        root: String,
        bind: SocketAddr,
        user_agent: String,
        #[serde(default = "default_cors_host")]
        cors_host: String,
        frontend_url: Url,
    },
    db {
        url: Url,
        password_path: PathBuf,
    },
    nadeo {
        identifier: String,
        secret_path: PathBuf,
        redirect_url: Url,
    }
);

fn default_cors_host() -> String {
    String::from("*")
}

static CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    let Ok(secret) = std::fs::read_to_string(&CONFIG.nadeo.secret_path) else {
        panic!("Couldn't read nadeo client secret file");
    };
    secret
});

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let arg = std::env::args().nth(1).expect("need config filename arg");
    let content = std::fs::read_to_string(arg).expect("could not read config file");

    let mut config = toml::from_str::<Config>(&content).expect("invalid TOML");
    config.hydrate_from_env();

    assert!(
        config.db.password_path.is_file(),
        "DB password path must be file"
    );

    config
        .db
        .url
        .set_username("root")
        .expect("Couldn't set username on DB URL");

    let Ok(password) = std::fs::read_to_string(&config.db.password_path) else {
        panic!("Couldn't read DB password file");
    };

    config
        .db
        .url
        .set_password(Some(password.trim()))
        .expect("Couldn't set password on DB URL");

    config
});

impl Config {
    fn route_v1(&self, path: &str) -> String {
        format!("{}/v1/{}", self.net.root, path)
    }
}

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Bind on {}", CONFIG.net.bind);

    let pool = MySqlPool::connect(CONFIG.db.url.as_str())
        .await
        .context("Connecting to database")?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Running migrations")?;

    drop(CLIENT_SECRET.clone());

    let app = Router::new()
        .route(&CONFIG.route_v1("map_data/{map_id}"), get(map_data))
        .route(&CONFIG.route_v1("oauth"), post(oauth))
        .fallback(fallback)
        .with_state(AppState { pool })
        .layer(
            CorsLayer::new()
                .allow_methods(cors::Any)
                .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?)
                .allow_headers(cors::Any),
        );

    let listener = TcpListener::bind(CONFIG.net.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fallback(uri: Uri) -> ApiError {
    ApiErrorInner::NotFound(uri.to_string()).into()
}

#[derive(Deserialize)]
struct OauthCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct NadeoOauthResponse {
    token_type: String,
    expires_in: u64,
    access_token: String,
    refresh_token: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct OauthResponse {
    access_token: String,
    refresh_token: String,
}

async fn oauth(Json(OauthCode { code }): Json<OauthCode>) -> Result<Json<OauthResponse>, ApiError> {
    let body_url_because_url_crate_doesnt_expose_params_parser_fuck = Url::parse_with_params(
        "h://a",
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &CONFIG.nadeo.identifier),
            ("client_secret", &CLIENT_SECRET),
            ("code", &code),
            ("redirect_uri", CONFIG.nadeo.redirect_url.as_str()),
        ],
    )
    .context("Parsing URL for access token request")?;

    let response = reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        .build()?
        .post(Url::parse("https://api.trackmania.com/api/access_token").unwrap())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(
            body_url_because_url_crate_doesnt_expose_params_parser_fuck
                .query()
                .unwrap()
                .to_owned(),
        )
        .send()
        .await
        .context("Sending request for access token")?;

    if response.status().is_success() {
        let response: NadeoOauthResponse = response.json().await?;
        Ok(Json(OauthResponse {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
        }))
    } else {
        let json_error: serde_json::Value = response.json().await?;
        Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataResponse {
    name: String,
}

async fn map_data(
    State(state): State<AppState>,
    Path(map_id): Path<String>,
) -> Result<Json<MapDataResponse>, ApiError> {
    let map_id: u64 = map_id.parse().context("Parsing map ID")?;

    let row = sqlx::query!("SELECT * FROM map WHERE ap_id = ?", map_id)
        .fetch_optional(&state.pool)
        .await
        .with_context(|| format!("Fetching map {map_id} from database"))?;

    if let Some(row) = row {
        Ok(Json(MapDataResponse { name: row.mapname }))
    } else {
        Err(ApiErrorInner::MapNotFound(map_id).into())
    }
}

#[derive(Debug, thiserror::Error, strum::IntoStaticStr, strum::VariantNames, TS)]
#[ts(export)]
pub enum ApiErrorInner {
    #[error("Database error: {0}")]
    Database(
        #[from]
        #[ts(skip)]
        sqlx::Error,
    ),

    #[error("Migration error: {0}")]
    Migration(
        #[from]
        #[ts(skip)]
        sqlx::migrate::MigrateError,
    ),

    #[error("Invalid map ID: {0}")]
    InvalidMapId(
        #[from]
        #[ts(skip)]
        std::num::ParseIntError,
    ),

    #[error("URL parse error: {0}")]
    UrlParseError(
        #[from]
        #[ts(skip)]
        url::ParseError,
    ),

    #[error("Session error: {0}")]
    SessionError(
        #[from]
        #[ts(skip)]
        tower_sessions::session::Error,
    ),

    #[error("Request to Nadeo API failed")]
    NadeoApiFailed(
        #[from]
        #[ts(skip)]
        reqwest::Error,
    ),

    #[error("Oauth failed: {0}")]
    OauthFailed(#[ts(skip)] String),

    #[error("Map not found: {0}")]
    MapNotFound(#[ts(skip)] u64),

    #[error("No such API: {0}")]
    NotFound(#[ts(skip)] String),
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
            | ApiErrorInner::SessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorInner::NadeoApiFailed(_) => StatusCode::BAD_GATEWAY,
            ApiErrorInner::InvalidMapId(_) | ApiErrorInner::OauthFailed(_) => {
                StatusCode::BAD_REQUEST
            }
            ApiErrorInner::MapNotFound(_) | ApiErrorInner::NotFound(_) => StatusCode::NOT_FOUND,
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

trait Context<T> {
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
