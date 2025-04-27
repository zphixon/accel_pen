use std::sync::LazyLock;

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, Method, Uri},
    response::{Html, Redirect},
    routing::get,
    Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use session::{AuthenticatedSession, RandomState, TokenPair};
use sqlx::{ConnectOptions, MySqlPool};
use tokio::net::TcpListener;
use tower_http::cors::{self, CorsLayer};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, Session, SessionManagerLayer,
};
use tracing_subscriber::EnvFilter;
use ts_rs::TS;
use url::Url;
use uuid::Uuid;

mod config;
mod error;
mod nadeo;
mod session;

use config::{CONFIG, OAUTH_CLIENT_SECRET};
use error::{ApiError, ApiErrorInner, Context};
use session::NadeoTokenPair;

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.with_file(true)
        //.with_line_number(true)
        .init();

    tracing::info!("Bind on {}", CONFIG.net.bind);

    let ubi_auth_task = tokio::spawn(nadeo::ubi_auth_task());

    let server_task = tokio::spawn(async {
        while nadeo::UBI_TOKENS.read().await.is_none() {
            tokio::task::yield_now().await;
        }

        let pool = MySqlPool::connect(CONFIG.db.url.as_str())
            .await
            .context("Connecting to database")?;

        sqlx::migrate!()
            .run(&pool)
            .await
            .context("Running migrations")?;

        let session_store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_same_site(SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(Duration::days(1)))
            .with_http_only(true)
            .with_private(Key::generate());

        let app = Router::new()
            .route(&CONFIG.route_v1("map_data"), get(map_data))
            .route(&CONFIG.route_v1("oauth/start"), get(oauth_start))
            .route(&CONFIG.route_v1("oauth/finish"), get(oauth_finish))
            .route(&CONFIG.route_v1("self"), get(self_handler))
            .with_state(AppState { pool })
            .fallback(fallback)
            .layer(
                CorsLayer::new()
                    .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?)
                    .allow_methods(cors::AllowMethods::list([Method::GET, Method::POST]))
                    .allow_credentials(true),
            )
            .layer(session_layer);

        let listener = TcpListener::bind(CONFIG.net.bind).await?;
        axum::serve(listener, app).await?;

        Ok::<_, anyhow::Error>(())
    });

    tokio::select! {
        serve_result = server_task => {
            serve_result??;
        },
        ubi_auth_task_result = ubi_auth_task => {
            ubi_auth_task_result??;
        },
    }

    Ok(())
}

async fn fallback(uri: Uri) -> ApiError {
    ApiErrorInner::NotFound(uri.to_string()).into()
}

async fn oauth_start(session: Session) -> Result<Redirect, ApiError> {
    session.clear().await;

    let state = Uuid::new_v4();
    RandomState::update_session(&session, state)
        .await
        .context("Updating session with random state")?;

    Ok(Redirect::to(
        Url::parse_with_params(
            nadeo::OAUTH_AUTHORIZE_URL,
            &[
                ("response_type", "code"),
                ("client_id", &CONFIG.nadeo.oauth.identifier),
                ("scope", "read_favorite write_favorite"),
                ("redirect_uri", CONFIG.nadeo.oauth.redirect_url.as_str()),
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
    random_state: RandomState,
    WithRejection(Query(request), _): WithRejection<Query<OauthFinishRequest>, ApiError>,
) -> Result<Html<&'static str>, ApiError> {
    if random_state.state().hyphenated().to_string() != request.state {
        return Err(ApiErrorInner::OauthFailed(String::from(
            "Invalid random state returned from Nadeo API",
        ))
        .into());
    }

    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("client_id", &CONFIG.nadeo.oauth.identifier)
        .append_pair("client_secret", &OAUTH_CLIENT_SECRET)
        .append_pair("code", &request.code)
        .append_pair("redirect_uri", CONFIG.nadeo.oauth.redirect_url.as_str())
        .finish();

    let response = nadeo::CLIENT
        .clone()
        .post(Url::parse(nadeo::OAUTH_GET_ACCESS_TOKEN_URL).unwrap())
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(params)
        .send()
        .await
        .context("Sending request for access token")?;

    if response.status().is_success() {
        let nadeo_oauth: NadeoTokenPair = response
            .json()
            .await
            .context("Parsing oauth tokens from Nadeo")?;

        AuthenticatedSession::update_session(
            random_state.session(),
            TokenPair::from_nadeo(nadeo_oauth),
        )
        .await
        .context("Finishing oauth")?;

        static CLIENT_REDIRECT: LazyLock<String> = LazyLock::new(|| {
            format!(
                r#"<!DOCTYPE html>
<html>
<head><meta http-equiv="refresh" content="0; url='{}'"></head>
<body></body>
</html>
"#,
                CONFIG.net.frontend_url.as_str()
            )
        });

        //Ok(Redirect::to(CONFIG.net.frontend_url.as_str()))
        Ok(Html(CLIENT_REDIRECT.as_str()))
    } else {
        let json_error: serde_json::Value = response.json().await?;
        Err(ApiErrorInner::OauthFailed(format!("{}", json_error)).into())
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct SelfResponse {
    display_name: String,
    account_id: String,
    club_tag: String,
}

async fn self_handler(auth_session: AuthenticatedSession) -> Result<Json<SelfResponse>, ApiError> {
    let user = nadeo::User::get(auth_session.tokens()).await?;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ClubTagResponse {
        club_tag: String,
    }

    let response = nadeo::CLIENT
        .clone()
        .get(
            Url::parse_with_params(
                "https://prod.trackmania.core.nadeo.online/accounts/clubTags/",
                &[("accountIdList", user.account_id.as_str())],
            )
            .context("Forming URL for request to get club tag")?,
        )
        .header(
            "Authorization",
            format!(
                "nadeo_v1 t={}",
                nadeo::UBI_TOKENS
                    .read()
                    .await
                    .as_ref()
                    .unwrap()
                    .nadeo_services
                    .access_token
                    .as_str()
            ),
        )
        .send()
        .await
        .context("Sending request for club tag")?
        .json::<Vec<ClubTagResponse>>()
        .await
        .context("Reading JSON for club tag response")?
        .pop()
        .unwrap();

    Ok(Json(SelfResponse {
        display_name: user.display_name,
        account_id: user.account_id,
        club_tag: response.club_tag,
    }))
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataRequest {
    map_id: u32,
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapDataResponse {
    name: String,
}

async fn map_data(
    State(state): State<AppState>,
    WithRejection(Query(request), _): WithRejection<Query<MapDataRequest>, ApiError>,
) -> Result<Json<MapDataResponse>, ApiError> {
    let row = sqlx::query!("SELECT * FROM map WHERE ap_id = ?", request.map_id)
        .fetch_optional(&state.pool)
        .await
        .with_context(|| format!("Fetching map {} from database", request.map_id))?;

    if let Some(row) = row {
        Ok(Json(MapDataResponse { name: row.mapname }))
    } else {
        Err(ApiErrorInner::MapNotFound(request.map_id).into())
    }
}
