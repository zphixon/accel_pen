use api::{ClubTag, FavoriteMaps, User};
use axum::{
    extract::{Query, State},
    http::{HeaderValue, Method, Uri},
    response::{Html, Redirect},
    routing::get,
    Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::sync::LazyLock;
use tokio::net::TcpListener;
use tower_http::cors::{AllowMethods, CorsLayer};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, Session, SessionManagerLayer,
};
use tracing_subscriber::EnvFilter;
use ts_rs::TS;
use uuid::Uuid;

mod api;
mod auth;
mod config;
mod error;

use auth::{
    nadeo::{NadeoAuthenticatedSession, NadeoTokens, RandomStateSession},
    ubi::UbiTokens,
};
use config::CONFIG;
use error::{ApiError, ApiErrorInner, Context};

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

    let ubi_auth_task = tokio::spawn(UbiTokens::auth_task());

    let server_task = tokio::spawn(async {
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
            .route(&CONFIG.route_v1("self/favorite_maps"), get(favorite_maps))
            .with_state(AppState { pool })
            .fallback(fallback)
            .layer(
                CorsLayer::new()
                    .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?)
                    .allow_methods(AllowMethods::list([Method::GET, Method::POST]))
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
    RandomStateSession::update_session(&session, state)
        .await
        .context("Updating session with random state")?;

    Ok(Redirect::to(auth::nadeo::oauth_start_url(state)?.as_str()))
}

#[derive(Deserialize)]
struct OauthFinishRequest {
    code: String,
    state: String,
}

async fn oauth_finish(
    random_state: RandomStateSession,
    WithRejection(Query(request), _): WithRejection<Query<OauthFinishRequest>, ApiError>,
) -> Result<Html<&'static str>, ApiError> {
    let token_pair = NadeoTokens::from_random_state_session(&random_state, request).await?;

    NadeoAuthenticatedSession::upgrade(&random_state, token_pair)
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
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct UserResponse {
    display_name: String,
    account_id: String,
    club_tag: String,
}

async fn self_handler(
    auth_session: NadeoAuthenticatedSession,
) -> Result<Json<UserResponse>, ApiError> {
    let club_tag = ClubTag::get_self(&auth_session).await?;
    Ok(Json(UserResponse {
        display_name: auth_session.display_name().to_owned(),
        account_id: auth_session.account_id().to_owned(),
        club_tag: club_tag.club_tag,
    }))
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct FavoriteMapResponse {
    uid: String,
    name: String,
    author: UserResponse,
}

async fn favorite_maps(
    auth_session: NadeoAuthenticatedSession,
) -> Result<Json<Vec<FavoriteMapResponse>>, ApiError> {
    let favorite_maps = FavoriteMaps::get(&auth_session).await?;

    let mut favorites = Vec::new();
    for favorite in favorite_maps.list {
        let user = User::get_from_account_id(&auth_session, &favorite.author).await?;
        let club_tag = ClubTag::get(&favorite.author).await?;
        favorites.push(FavoriteMapResponse {
            uid: favorite.uid,
            name: favorite.name,
            author: UserResponse {
                display_name: user.display_name,
                account_id: user.account_id,
                club_tag: club_tag.club_tag,
            },
        });
    }

    Ok(Json(favorites))
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
