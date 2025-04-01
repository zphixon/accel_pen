use axum::{extract::State, http::StatusCode, routing::get, Router};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, MySqlPool};
use std::{net::SocketAddr, path::PathBuf, sync::LazyLock};
use tokio::net::TcpListener;
use url::Url;

#[derive(Deserialize)]
struct Config {
    net: NetConfig,
    db: DbConfig,
}

#[derive(Deserialize)]
struct NetConfig {
    root: String,
    bind: SocketAddr,
    user_agent: String,
}

#[derive(Deserialize)]
struct DbConfig {
    url: Url,
    password_path: PathBuf,
}

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let arg = std::env::args().nth(1).expect("need config filename arg");
    let content = std::fs::read_to_string(arg).expect("could not read config file");

    let mut config = toml::from_str::<Config>(&content).expect("invalid TOML");
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
        .set_password(Some(&password))
        .expect("Couldn't set password on DB URL");

    config
});

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Bind on {}", CONFIG.net.bind);

    let pool = MySqlPool::connect(CONFIG.db.url.as_str()).await?;

    let app = Router::new()
        .route(&CONFIG.net.root, get(hello))
        .fallback(fallback)
        .with_state(AppState { pool });
    let listener = TcpListener::bind(CONFIG.net.bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fallback() -> (StatusCode, String) {
    ErrorBacktrace::response_status_anyhow(
        StatusCode::NOT_FOUND,
        anyhow::anyhow!("No such endpoint"),
    )
}

async fn hello(State(state): State<AppState>) -> Result<String, (StatusCode, String)> {
    let row = sqlx::query!("SELECT * FROM map")
        .fetch_optional(&state.pool)
        .await
        .map_err(ErrorBacktrace::internal_server)?;

    if let Some(row) = row {
        Ok(format!("got a row: {}\n", row.mapname))
    } else {
        Ok(String::from("Hello!\n"))
    }
}

#[derive(Serialize)]
struct ErrorBacktrace {
    error: String,
    backtrace: String,
}

impl ErrorBacktrace {
    fn response_status_anyhow(
        status: StatusCode,
        anyhow_err: anyhow::Error,
    ) -> (StatusCode, String) {
        let error = anyhow_err.to_string();
        let backtrace = anyhow_err.backtrace().to_string();

        tracing::error!("{}\n{}", backtrace, error);
        let err = ErrorBacktrace { error, backtrace };

        (
            status,
            serde_json::to_string(&err).expect("error to string hmmmm"),
        )
    }

    fn response_status(
        status: StatusCode,
        err: impl std::error::Error + Send + Sync + 'static,
    ) -> (StatusCode, String) {
        let anyhow_err = anyhow::Error::from(err);
        Self::response_status_anyhow(status, anyhow_err)
    }

    fn internal_server(
        err: impl std::error::Error + Send + Sync + 'static,
    ) -> (StatusCode, String) {
        Self::response_status(StatusCode::INTERNAL_SERVER_ERROR, err)
    }
}
