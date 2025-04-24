use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::MySqlPool;
use std::{fmt::Display, net::SocketAddr, ops::Deref, path::PathBuf, sync::LazyLock};
use tokio::net::TcpListener;
use tower_http::cors::{self, CorsLayer};
use url::Url;

from_env::config!(
    "ACCEL_PEN",
    net {
        root: String,
        bind: SocketAddr,
        user_agent: String,
        cors_host: String,
    },
    db {
        url: Url,
        password_path: PathBuf,
        test_migrations: Option<bool>,
    }
);

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
    fn route(&self, path: &str) -> String {
        format!("{}/{}", self.net.root, path)
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

    let pool = MySqlPool::connect(CONFIG.db.url.as_str()).await?;
    sqlx::migrate!().run(&pool).await?;
    if CONFIG.db.test_migrations.unwrap_or(false) {
        sqlx::migrate!("./migrations_test").run(&pool).await?;
    }

    let app = Router::new()
        .route(&CONFIG.route("map_data/{map_id}"), get(map_data))
        .fallback(fallback)
        .with_state(AppState { pool })
        .layer(
            CorsLayer::new()
                .allow_methods(cors::Any)
                .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?),
        );

    let listener = TcpListener::bind(CONFIG.net.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fallback(uri: Uri) -> ApiError {
    ApiErrorInner::NotFound(uri).into()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
pub enum ApiErrorInner {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Invalid map ID")]
    InvalidMapId(#[from] std::num::ParseIntError),

    #[error("Map not found: {0}")]
    MapNotFound(u64),

    #[error("No such API: {0:?}")]
    NotFound(Uri),
}

#[derive(Debug)]
pub enum ApiError {
    Root(ApiErrorInner),
    Context {
        context: String,
        inner: Box<ApiError>,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error: &'static str = (&*self).into();
        let status_code = match &*self {
            ApiErrorInner::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorInner::InvalidMapId(_) => StatusCode::BAD_REQUEST,
            ApiErrorInner::MapNotFound(_) | ApiErrorInner::NotFound(_) => StatusCode::NOT_FOUND,
        };

        #[derive(Serialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        struct ApiError {
            error: String,
            message: String,
        }

        (
            status_code,
            Json(ApiError {
                error: error.to_owned(),
                message: self.to_string(),
            }),
        )
            .into_response()
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
