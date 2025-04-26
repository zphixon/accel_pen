use axum::{
    body::Body,
    error_handling::HandleErrorLayer,
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Path, Query, Request, State,
    },
    http::{header, HeaderValue, StatusCode, Uri},
    middleware::{self, Next},
    response::{AppendHeaders, IntoResponse, Redirect, Response},
    routing::{get, post},
    BoxError, Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::{
    collections::HashMap,
    fmt::Display,
    net::SocketAddr,
    ops::Deref,
    path::PathBuf,
    sync::{Arc, LazyLock},
};
use tokio::{net::TcpListener, sync::RwLock};
use tower_http::cors::{self, CorsLayer};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, Session, SessionManagerLayer,
};
use ts_rs::TS;
use url::Url;
use uuid::Uuid;

from_env::config!(
    "ACCEL_PEN",
    net {
        domain: String,
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

struct Login {
    csrf_token: Uuid,
    nadeo_oauth: NadeoOauth,
}

#[derive(Debug, Deserialize)]
struct NadeoOauth {
    token_type: String,
    expires_in: u64,
    access_token: String,
    refresh_token: String,
}

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    sessions: Arc<RwLock<HashMap<Uuid, Login>>>,
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

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_same_site(SameSite::Strict)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)))
        .with_http_only(true)
        .with_private(Key::generate());

    let app = Router::new()
        .route(&CONFIG.route_v1("map_data"), get(map_data))
        .route(&CONFIG.route_v1("oauth/start"), get(oauth_start))
        .route(&CONFIG.route_v1("oauth/finish"), get(oauth_finish))
        .with_state(AppState {
            pool,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
        .fallback(fallback)
        .layer(
            CorsLayer::new()
                .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?)
                .allow_methods(cors::Any)
                .allow_headers(cors::Any),
        )
        .layer(session_layer);

    let listener = TcpListener::bind(CONFIG.net.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fallback(uri: Uri) -> ApiError {
    ApiErrorInner::NotFound(uri.to_string()).into()
}

const RANDOM_STATE_KEY: &str = "randomState";
const SESSSION_ID_KEY: &str = "sessionId";
const CSRF_TOKEN_KEY: &str = "csrfToken";

async fn oauth_start(session: Session) -> Result<Redirect, ApiError> {
    session
        .insert(SESSSION_ID_KEY, Uuid::new_v4())
        .await
        .context("Setting extra entropy on session")?;

    let state = Uuid::new_v4();
    session
        .insert(RANDOM_STATE_KEY, state)
        .await
        .context("Setting random state on session")?;

    Ok(Redirect::to(
        Url::parse_with_params(
            "https://api.trackmania.com/oauth/authorize",
            &[
                ("response_type", "code"),
                ("client_id", &CONFIG.nadeo.identifier),
                ("scope", "read_favorite write_favorite"),
                ("redirect_uri", CONFIG.nadeo.redirect_url.as_str()),
                ("state", state.as_hyphenated().to_string().as_str()),
            ],
        )
        .context("Creating redirect URL to Nadeo")?
        .as_str(),
    ))
}

#[derive(Deserialize)]
struct OauthFinishRequest {
    code: String,
    state: String,
}

async fn oauth_finish(
    State(state): State<AppState>,
    session: Session,
    WithRejection(Query(request), _): WithRejection<Query<OauthFinishRequest>, ApiError>,
) -> Result<Redirect, ApiError> {
    let Some(session_id) = session
        .get::<Uuid>(SESSSION_ID_KEY)
        .await
        .context("Reading session ID from session")?
    else {
        return Err(
            ApiErrorInner::OauthFailed(String::from("Missing session ID in session")).into(),
        );
    };

    let Some(session_state) = session
        .get::<Uuid>(RANDOM_STATE_KEY)
        .await
        .context("Reading random state from session")?
    else {
        return Err(
            ApiErrorInner::OauthFailed(String::from("Missing random state in session")).into(),
        );
    };
    if session_state.hyphenated().to_string() != request.state {
        return Err(ApiErrorInner::OauthFailed(String::from(
            "Invalid random state returned from Nadeo API",
        ))
        .into());
    }

    let url_crate_doesnt_expose_params_parser = Url::parse_with_params(
        "h://a",
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &CONFIG.nadeo.identifier),
            ("client_secret", &CLIENT_SECRET),
            ("code", &request.code),
            ("redirect_uri", CONFIG.nadeo.redirect_url.as_str()),
        ],
    )
    .context("Parsing URL for access token request")?;

    let response = reqwest::Client::builder()
        .user_agent(&CONFIG.net.user_agent)
        .build()?
        .post(Url::parse("https://api.trackmania.com/api/access_token").unwrap())
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(
            url_crate_doesnt_expose_params_parser
                .query()
                .unwrap()
                .to_owned(),
        )
        .send()
        .await
        .context("Sending request for access token")?;

    if response.status().is_success() {
        let nadeo_oauth = response.json().await?;

        let csrf_token = Uuid::new_v4();
        state.sessions.write().await.insert(
            session_id,
            Login {
                csrf_token,
                nadeo_oauth,
            },
        );

        Ok(Redirect::to(
            Url::parse_with_params(
                CONFIG.net.frontend_url.as_str(),
                &[(CSRF_TOKEN_KEY, csrf_token.hyphenated().to_string().as_str())],
            )
            .context("Creating redirect URL back to frontend")?
            .as_str(),
        ))
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

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataRequest {
    id: u64,
}

async fn map_data(
    State(state): State<AppState>,
    WithRejection(Query(map_id), _): WithRejection<Query<MapDataRequest>, ApiError>,
) -> Result<Json<MapDataResponse>, ApiError> {
    let map_id: u64 = map_id.id;

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
        QueryRejection,
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

    #[error("Request to Nadeo API failed")]
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
        JsonRejection,
    ),

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
            | ApiErrorInner::SessionError(_)
            | ApiErrorInner::AxumError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorInner::NadeoApiFailed(_) => StatusCode::BAD_GATEWAY,
            ApiErrorInner::InvalidMapDataRequest(_)
            | ApiErrorInner::OauthFailed(_)
            | ApiErrorInner::InvalidOauth(_) => StatusCode::BAD_REQUEST,
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
