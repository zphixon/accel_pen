use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, Uri},
    response::Redirect,
    routing::get,
    Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tokio::net::TcpListener;
use tower_http::cors::{self, CorsLayer};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, Session, SessionManagerLayer,
};
use ts_rs::TS;
use url::Url;
use uuid::Uuid;

mod config;
mod error;

use config::{CLIENT_SECRET, CONFIG};
use error::{ApiError, ApiErrorInner, Context};

#[derive(Debug, Serialize, Deserialize)]
struct NadeoOauth {
    token_type: String,
    expires_in: u64,
    access_token: String,
    refresh_token: String,
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
        .with_state(AppState { pool })
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
const NADEO_TOKENS_KEY: &str = "nadeoTokens";

async fn oauth_start(session: Session) -> Result<Redirect, ApiError> {
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
    session: Session,
    WithRejection(Query(request), _): WithRejection<Query<OauthFinishRequest>, ApiError>,
) -> Result<Redirect, ApiError> {
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
        let nadeo_oauth: NadeoOauth = response
            .json()
            .await
            .context("Parsing oauth tokens from Nadeo")?;

        session
            .insert(NADEO_TOKENS_KEY, nadeo_oauth)
            .await
            .context("Writing oauth tokens to session")?;

        Ok(Redirect::to(CONFIG.net.frontend_url.as_str()))
    } else {
        let json_error: serde_json::Value = response.json().await?;
        Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
    }
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataRequest {
    id: u64,
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataResponse {
    name: String,
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
