use api::{ClubTag, FavoriteMaps, User};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderValue, Method, Uri},
    response::{Html, Redirect},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tokio::net::TcpListener;
use tower_http::{
    cors::{AllowMethods, CorsLayer},
    limit::RequestBodyLimitLayer,
};
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
            .route(&CONFIG.route_v1("map/data"), get(map_data))
            .route(&CONFIG.route_v1("map/upload"), post(map_upload))
            .route(&CONFIG.route_v1("oauth/start"), get(oauth_start))
            .route(&CONFIG.route_v1("oauth/finish"), get(oauth_finish))
            .route(&CONFIG.route_v1("oauth/logout"), get(oauth_logout))
            .route(&CONFIG.route_v1("self"), get(self_handler))
            .route(&CONFIG.route_v1("self/favorite_maps"), get(favorite_maps))
            .with_state(AppState { pool })
            .fallback(fallback)
            .layer(session_layer)
            .layer(DefaultBodyLimit::disable())
            .layer(RequestBodyLimitLayer::new(20 * 1000 * 1000))
            .layer(
                CorsLayer::new()
                    .allow_origin(CONFIG.net.cors_host.parse::<HeaderValue>()?)
                    .allow_methods(AllowMethods::list([Method::GET, Method::POST]))
                    .allow_credentials(true),
            );

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

#[derive(Deserialize)]
struct OauthStartRequest {
    return_path: Option<String>,
}

async fn oauth_start(
    session: Session,
    WithRejection(Query(request), _): WithRejection<Query<OauthStartRequest>, ApiError>,
) -> Result<Redirect, ApiError> {
    session.clear().await;

    let state = Uuid::new_v4();
    RandomStateSession::update_session(&session, state, request.return_path)
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
    State(state): State<AppState>,
    random_state: RandomStateSession,
    WithRejection(Query(request), _): WithRejection<Query<OauthFinishRequest>, ApiError>,
) -> Result<Html<String>, ApiError> {
    let token_pair = NadeoTokens::from_random_state_session(&random_state, request).await?;

    let session = NadeoAuthenticatedSession::upgrade(random_state, token_pair)
        .await
        .context("Finishing oauth")?;

    sqlx::query!(
        "
            INSERT INTO user (display_name, account_id, registered)
            VALUES (?, ?, NOW())
            ON DUPLICATE KEY UPDATE display_name=display_name
        ",
        session.display_name(),
        session.account_id(),
    )
    .execute(&state.pool)
    .await
    .context("Adding user to users table")?;

    let mut frontend_url = CONFIG.net.frontend_url.clone();
    {
        let mut segments = frontend_url.path_segments_mut().unwrap();
        if let Some(return_path) = session.return_path() {
            for part in return_path.split("/") {
                if part != "" {
                    segments.push(part);
                }
            }
            tracing::trace!("Returning to {:?}", return_path);
        }
    }

    let client_redirect = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta http-equiv="refresh" content="0; url='{}'"></head>
<body></body>
</html>
"#,
        frontend_url
    );

    //Ok(Redirect::to(CONFIG.net.frontend_url.as_str()))
    Ok(Html(client_redirect))
}

async fn oauth_logout(auth_session: NadeoAuthenticatedSession) -> Result<Redirect, ApiError> {
    auth_session.session().clear().await;
    Ok(Redirect::to(CONFIG.net.frontend_url.as_str()))
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct UserResponse {
    display_name: String,
    account_id: String,
    user_id: u32,
    club_tag: String,
}

async fn self_handler(
    State(state): State<AppState>,
    auth_session: NadeoAuthenticatedSession,
) -> Result<Json<UserResponse>, ApiError> {
    let Some(user_id) = sqlx::query!(
        "SELECT user_id FROM user WHERE account_id = ?",
        auth_session.account_id()
    )
    .fetch_optional(&state.pool)
    .await
    .context("Finding Accel Pen account for favorite map")?
    else {
        return Err(ApiErrorInner::NotFound(String::from("Self not found in DB?")).into());
    };

    let club_tag = ClubTag::get_self(&auth_session)
        .await
        .context("Get self club tag")?;
    Ok(Json(UserResponse {
        display_name: auth_session.display_name().to_owned(),
        account_id: auth_session.account_id().to_owned(),
        club_tag: club_tag.club_tag,
        user_id: user_id.user_id,
    }))
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct FavoriteMapResponse {
    uid: String,
    name: String,
    author: UserOrAuthor,
}

#[derive(Serialize, TS)]
#[serde(untagged)]
enum UserOrAuthor {
    User(UserResponse),
    Author(AuthorResponse),
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct AuthorResponse {
    account_id: String,
    display_name: String,
    club_tag: String,
}

async fn favorite_maps(
    State(state): State<AppState>,
    auth_session: NadeoAuthenticatedSession,
) -> Result<Json<Vec<FavoriteMapResponse>>, ApiError> {
    let favorite_maps = FavoriteMaps::get(&auth_session)
        .await
        .context("Getting favorite maps")?;

    let mut favorites = Vec::new();
    for favorite in favorite_maps.list {
        let user = User::get_from_account_id(&auth_session, &favorite.author)
            .await
            .context("Get user from account ID for favorite map author")?;
        let club_tag = ClubTag::get(&favorite.author)
            .await
            .context("Get club tag for favorite map author")?;

        favorites.push(FavoriteMapResponse {
            uid: favorite.uid,
            name: favorite.name,
            author: if let Some(user_id) = sqlx::query!(
                "SELECT user_id FROM user WHERE account_id = ?",
                favorite.author
            )
            .fetch_optional(&state.pool)
            .await
            .context("Finding Accel Pen account for favorite map")?
            {
                UserOrAuthor::User(UserResponse {
                    display_name: user.display_name,
                    account_id: user.account_id,
                    club_tag: club_tag.club_tag,
                    user_id: user_id.user_id,
                })
            } else {
                UserOrAuthor::Author(AuthorResponse {
                    account_id: user.account_id,
                    display_name: user.display_name,
                    club_tag: club_tag.club_tag,
                })
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
    author: UserResponse,
    uploaded: String,
}

async fn map_data(
    State(state): State<AppState>,
    WithRejection(Query(request), _): WithRejection<Query<MapDataRequest>, ApiError>,
) -> Result<Json<MapDataResponse>, ApiError> {
    let row = sqlx::query!(
        "
            SELECT map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.author, user.display_name, user.user_id, user.account_id
            FROM map JOIN user ON map.author = user.user_id
            WHERE map.ap_id = ?
        ",
        request.map_id
    )
    .fetch_optional(&state.pool)
    .await
    .with_context(|| format!("Fetching map {} from database", request.map_id))?;

    if let Some(row) = row {
        let club_tag = ClubTag::get(&row.account_id)
            .await
            .context("Getting club tag for map data author")?;
        Ok(Json(MapDataResponse {
            name: row.mapname,
            author: UserResponse {
                display_name: row.display_name,
                account_id: row.account_id,
                user_id: row.user_id,
                club_tag: club_tag.club_tag,
            },
            uploaded: row
                .uploaded
                .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
                .context("Formatting map upload time")?,
        }))
    } else {
        Err(ApiErrorInner::MapNotFound(request.map_id).into())
    }
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapUploadMeta {}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapUploadResponse {
    map_id: u32,
}

async fn map_upload(
    State(state): State<AppState>,
    session: NadeoAuthenticatedSession,
    WithRejection(mut multipart, _): WithRejection<Multipart, ApiError>,
) -> Result<Json<MapUploadResponse>, ApiError> {
    let user_id = sqlx::query!(
        "SELECT user_id FROM user WHERE account_id = ?",
        session.account_id()
    )
    .fetch_one(&state.pool)
    .await
    .context("Reading numeric user ID from database")?;

    let mut map_data = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .context("Reading field from multipart body while uploading map")?
    {
        let Some(name) = field.name() else {
            return Err(ApiErrorInner::MissingFromMultipart("Name of field").into());
        };
        let name = String::from(name);
        let data = field
            .bytes()
            .await
            .with_context(|| format!("Reading value of multipart field {}", name))?;

        tracing::debug!("{:?}", name);

        if &name == "map_meta" {}
        if &name == "map_data" {
            map_data = Some(data);
        }
    }

    let Some(map_data) = map_data else {
        return Err(ApiErrorInner::MissingFromMultipart("Value of map_data").into());
    };

    let map_node = gbx_rs::Node::read_from(&map_data).context("Parsing map for upload")?;
    let gbx_rs::parse::CGame::CtnChallenge(map) =
        map_node.parse().context("Parsing full map data")?
    else {
        return Err(ApiErrorInner::NotAMap.into());
    };

    let Some(map_info) = map.map_info.as_ref() else {
        return Err(ApiErrorInner::MissingFromMultipart("Map info from map").into());
    };

    let author = auth::nadeo::login_to_uid(map_info.author).context("Parsing map author")?;
    if session.account_id() != author {
        return Err(ApiErrorInner::NotYourMap.into());
    }

    let maybe_map = sqlx::query!(
        "SELECT ap_id, gbx_mapuid FROM map WHERE gbx_mapuid = ?",
        map_info.id
    )
    .fetch_optional(&state.pool)
    .await
    .context("Trying to find a map that might already exist")?;

    if let Some(exists_map) = maybe_map {
        // hmmmmmmmmm
        return Err(ApiErrorInner::AlreadyUploaded {
            map_id: exists_map.ap_id,
        }
        .into());
    }

    let Some(map_name) = map.map_name else {
        return Err(ApiErrorInner::MissingFromMultipart("Map name").into());
    };

    let buffer = map_data.to_vec();
    sqlx::query!("INSERT INTO map (gbx_mapuid, gbx_data, mapname, author, created, uploaded) VALUES (?, ?, ?, ?, NOW(), NOW())",
        map_info.id,
        buffer,
        map_name,
        user_id.user_id,
    ).execute(&state.pool).await.context("Adding map to database")?;

    let ap_id = sqlx::query!("SELECT ap_id FROM map WHERE gbx_mapuid = ?", map_info.id)
        .fetch_one(&state.pool)
        .await
        .context("Retrieving ID of newly uploaded map")?;

    Ok(Json(MapUploadResponse {
        map_id: ap_id.ap_id,
    }))
}
