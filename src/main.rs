use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{collections::HashSet, sync::Arc};
use tera::Tera;
use tokio::net::TcpListener;
use tower_http::{
    cors::{AllowMethods, CorsLayer},
    limit::RequestBodyLimitLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
};
use tower_sessions::{
    cookie::{time::Duration, Key, SameSite},
    Expiry, MemoryStore, Session, SessionManagerLayer,
};
use tracing_subscriber::EnvFilter;
use ts_rs::TS;
use uuid::Uuid;

mod config;
mod dev;
mod error;
mod nadeo;
mod ubi;

use config::CONFIG;
use error::{ApiError, ApiErrorInner, Context};
use nadeo::auth::{NadeoAuthSession, NadeoOauthFinishRequest, RandomStateSession};
use ubi::UbiTokens;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    tera: Arc<std::sync::RwLock<Tera>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.with_file(true)
        //.with_line_number(true)
        .init();

    tracing::info!("Bind on {}", CONFIG.net.bind);

    let tera = Tera::new("frontend/templates/**/*").context("Reading templates")?;
    tracing::debug!("Template names:");
    for template_name in tera.get_template_names() {
        tracing::debug!("  {}", template_name);
    }
    let tera = Arc::new(std::sync::RwLock::new(tera));

    if CONFIG.dev_reload {
        dev::reload_task(Arc::clone(&tera));
    }

    let ubi_auth_task = tokio::spawn(UbiTokens::auth_task());

    let server_task = tokio::spawn(async {
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(CONFIG.db.url.as_str())
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

        let long_cache = SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=604800"),
        );

        let app = Router::new()
            .route(&CONFIG.route(""), get(index))
            .route(&CONFIG.route("map/{map_id}"), get(get_map_page))
            .route(
                &CONFIG.route_api_v1("map/{map_id}/thumbnail"),
                get(get_map_thumbnail).layer(long_cache),
            )
            .route(&CONFIG.route("map/upload"), get(get_map_upload))
            .route(&CONFIG.route_api_v1("map/upload"), post(post_map_upload))
            .route(&CONFIG.oauth_start_route(), get(oauth_start))
            .route(&CONFIG.oauth_finish_route(), get(oauth_finish))
            .route(&CONFIG.oauth_logout_route(), get(oauth_logout))
            .nest_service(&CONFIG.route("static"), ServeDir::new("frontend/static"))
            .with_state(AppState { pool, tera })
            .layer(session_layer)
            .layer(DefaultBodyLimit::disable())
            .layer(RequestBodyLimitLayer::new(20 * 1000 * 1000))
            .layer(
                CorsLayer::new()
                    .allow_origin(CONFIG.net.url.as_str().parse::<HeaderValue>()?)
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

#[derive(Serialize)]
struct MapContext<'auth> {
    id: i32,
    gbx_uid: &'auth str,
    plain_name: String,
    name: nadeo::FormattedString,
    votes: i32,
    uploaded: String,
    author: UserResponse,
    tags: Vec<TagInfo>,
}

async fn index(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    if let Some(auth) = auth {
        match sqlx::query!(
            "
                SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded
                FROM map
                WHERE map.ap_author_id = $1
                LIMIT 20
            ",
            auth.user_id(),
        )
        .fetch_all(&state.pool)
        .await
        {
            Ok(my_maps) => {
                let mut maps_context = Vec::new();
                for map in my_maps.iter() {
                    let name = nadeo::FormattedString::parse(&map.mapname);
                    maps_context.push(MapContext {
                        id: map.ap_map_id,
                        gbx_uid: &map.gbx_mapuid,
                        plain_name: name.strip_formatting(),
                        name,
                        votes: map.votes,
                        uploaded: map
                            .uploaded
                            .format(
                                &time::format_description::well_known::Iso8601::DATE_TIME_OFFSET,
                            )
                            .context("Formatting map upload time")
                            .expect(
                                "this is why i wanted to use regular Results for error handling",
                            ),
                        author: UserResponse {
                            display_name: auth.display_name().to_owned(),
                            account_id: auth.account_id().to_owned(),
                            user_id: auth.user_id(),
                            club_tag: auth.club_tag().map(nadeo::FormattedString::parse),
                        },
                        tags: vec![],
                    });
                }
                context.insert("my_maps", &maps_context);
            }
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
            }
        }
    }

    match sqlx::query!(
        "
            SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.ap_author_id,
                ap_user.nadeo_display_name, ap_user.nadeo_id, ap_user.nadeo_club_tag
            FROM map JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
            ORDER BY map.ap_map_id DESC
            LIMIT 10
        ",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(recent_rows) => {
            let mut recent_maps = Vec::new();
            for map in recent_rows.iter() {
                let name = nadeo::FormattedString::parse(&map.mapname);
                recent_maps.push(MapContext {
                    id: map.ap_map_id,
                    gbx_uid: &map.gbx_mapuid,
                    plain_name: name.strip_formatting(),
                    name,
                    votes: map.votes,
                    uploaded: map
                        .uploaded
                        .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
                        .context("Formatting map upload time")
                        .expect("this is why i wanted to use regular Results for error handling"),
                    author: UserResponse {
                        display_name: map.nadeo_display_name.clone(),
                        account_id: map.nadeo_id.clone(),
                        user_id: map.ap_author_id,
                        club_tag: map.nadeo_club_tag.as_deref().map(nadeo::FormattedString::parse),
                    },
                    tags: vec!(),
                });
            }
            context.insert("recent_maps", &recent_maps);
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Couldn't get recent maps: {}", err),
            )
                .into_response()
        }
    }

    match state
        .tera
        .read()
        .unwrap()
        .render("index.html.tera", &context)
        .context("Rendering index template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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

    Ok(Redirect::to(nadeo::auth::oauth_start_url(state)?.as_str()))
}

async fn oauth_finish(
    State(state): State<AppState>,
    random_state: RandomStateSession,
    WithRejection(Query(request), _): WithRejection<Query<NadeoOauthFinishRequest>, ApiError>,
) -> Result<Html<String>, ApiError> {
    let session = NadeoAuthSession::upgrade(&state, random_state, request)
        .await
        .context("Finishing oauth")?;

    let mut frontend_url = CONFIG.net.url.clone();
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
            .join(session.return_path().unwrap_or_default())
            .unwrap_or(frontend_url)
    );

    //Ok(Redirect::to(CONFIG.net.frontend_url.as_str()))
    Ok(Html(client_redirect))
}

async fn oauth_logout(auth_session: NadeoAuthSession) -> Result<Redirect, ApiError> {
    auth_session.session().clear().await;
    Ok(Redirect::to(CONFIG.net.url.as_str()))
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct UserResponse {
    display_name: String,
    account_id: String,
    user_id: i32,
    club_tag: Option<nadeo::FormattedString>,
}

//#[derive(Deserialize, TS)]
//#[ts(export)]
//#[serde(tag = "type")]
//struct MapDataRequest {
//    map_id: i32,
//}
//
//#[derive(Serialize, TS)]
//#[ts(export)]
//#[serde(tag = "type")]
//struct MapDataResponse {
//    name: String,
//    author: UserResponse,
//    uploaded: String,
//    map_id: i32,
//    uid: String,
//}
//
//async fn map_data(
//    State(state): State<AppState>,
//    WithRejection(Query(request), _): WithRejection<Query<MapDataRequest>, ApiError>,
//) -> Result<Json<MapDataResponse>, ApiError> {
//    let row = sqlx::query!(
//        "
//            SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.ap_author_id,
//                ap_user.nadeo_display_name, ap_user.ap_user_id, ap_user.nadeo_id, ap_user.nadeo_club_tag
//            FROM map JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
//            WHERE map.ap_map_id = $1
//        ",
//        request.map_id
//    )
//    .fetch_optional(&state.pool)
//    .await
//    .with_context(|| format!("Fetching map {} from database", request.map_id))?;
//
//    if let Some(row) = row {
//        Ok(Json(MapDataResponse {
//            name: row.mapname,
//            author: UserResponse {
//                display_name: row.nadeo_display_name,
//                account_id: row.nadeo_id,
//                user_id: row.ap_user_id,
//                club_tag: row.nadeo_club_tag,
//            },
//            uploaded: row
//                .uploaded
//                .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
//                .context("Formatting map upload time")?,
//            map_id: row.ap_map_id,
//            uid: row.gbx_mapuid,
//        }))
//    } else {
//        Err(ApiErrorInner::MapNotFound {
//            map_id: request.map_id,
//        }
//        .into())
//    }
//}

async fn get_map_page(
    State(state): State<AppState>,
    auth: Option<NadeoAuthSession>,
    Path(map_id): Path<i32>,
) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let map = match sqlx::query!(
        "
            SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded,
                map.ap_author_id, ap_user.nadeo_display_name, ap_user.ap_user_id, ap_user.nadeo_id,
                ap_user.nadeo_club_tag
            FROM map JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
            WHERE map.ap_map_id = $1
        ",
        map_id,
    )
    .fetch_optional(&state.pool)
    .await
    {
        Ok(maybe_row) => maybe_row,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", err),
            )
                .into_response()
        }
    };

    if let Some(map) = map {
        let tags = match sqlx::query!(
            "
            SELECT tag_name.tag_id, tag_name.tag_name, tag_name.tag_kind
            FROM tag_name
            JOIN tag ON tag.tag_id = tag_name.tag_id
            JOIN map ON tag.ap_map_id = $1
            GROUP BY tag_name.tag_id
            ORDER BY tag_name.tag_id ASC
        ",
            map.ap_map_id
        )
        .fetch_all(&state.pool)
        .await
        {
            Ok(tags) => tags,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Getting tags").into_response(),
        }
        .into_iter()
        .map(|row| TagInfo {
            id: row.tag_id,
            name: row.tag_name,
            kind: row.tag_kind,
        })
        .collect::<Vec<_>>();

        let name = nadeo::FormattedString::parse(&map.mapname);
        context.insert(
            "map",
            &MapContext {
                id: map.ap_map_id,
                gbx_uid: &map.gbx_mapuid,
                plain_name: name.strip_formatting(),
                name,
                votes: map.votes,
                uploaded: map
                    .uploaded
                    .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
                    .unwrap(),
                author: UserResponse {
                    display_name: map.nadeo_display_name,
                    account_id: map.nadeo_id,
                    user_id: map.ap_user_id,
                    club_tag: map.nadeo_club_tag.as_deref().map(nadeo::FormattedString::parse),
                },
                tags,
            },
        );
    } else {
        context.insert("map", &None::<()>);
    }

    match state
        .tera
        .read()
        .unwrap()
        .render("map/page.html.tera", &context)
        .context("Rendering map page template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_map_thumbnail(
    State(state): State<AppState>,
    Path(map_id): Path<i32>,
) -> Result<Response, ApiError> {
    Ok((
        [(header::CONTENT_TYPE, "image/jpeg")],
        sqlx::query!("SELECT thumbnail FROM map WHERE ap_map_id = $1", map_id)
            .fetch_one(&state.pool)
            .await
            .context("Reading thumbnail from database")?
            .thumbnail,
    )
        .into_response())
}

#[derive(Serialize, TS)]
#[ts(export)]
struct TagInfo {
    id: i32,
    name: String,
    kind: String,
}

async fn get_map_upload(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let tag_names = match sqlx::query!("SELECT tag_id, tag_name, tag_kind FROM tag_name")
        .fetch_all(&state.pool)
        .await
    {
        Ok(tag_names) => tag_names,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Getting names of tags").into_response()
        }
    };

    let tag_names = tag_names
        .into_iter()
        .map(|row| TagInfo {
            id: row.tag_id,
            name: row.tag_name,
            kind: row.tag_kind,
        })
        .collect::<Vec<TagInfo>>();
    context.insert("tags", &tag_names);

    match state
        .tera
        .read()
        .unwrap()
        .render("map/upload.html.tera", &context)
        .context("Rendering map upload template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapUploadResponse {
    map_id: i32,
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapUploadMeta {
    tags: Vec<String>,
}

async fn post_map_upload(
    State(state): State<AppState>,
    session: NadeoAuthSession,
    WithRejection(mut multipart, _): WithRejection<Multipart, ApiError>,
) -> Result<Json<MapUploadResponse>, ApiError> {
    let mut map_data = None;
    let mut map_meta = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .context("Reading field from multipart body while uploading map")?
    {
        let Some(name) = field.name() else {
            return Err(ApiErrorInner::MissingFromMultipart {
                error: "Name of field",
            }
            .into());
        };
        let name = String::from(name);
        let data = field
            .bytes()
            .await
            .with_context(|| format!("Reading value of multipart field {}", name))?;

        tracing::debug!("{:?}", name);

        if &name == "map_meta" {
            map_meta = Some(data);
        } else if &name == "map_data" {
            map_data = Some(data);
        }
    }

    let Some(map_data) = map_data else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Value of map_data",
        }
        .into());
    };

    let Some(map_meta) = map_meta else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Value of map meta",
        }
        .into());
    };

    let map_meta =
        serde_json::from_slice::<MapUploadMeta>(&map_meta).context("Parsing map meta as JSON")?;

    let mut map_tags: HashSet<i32> = HashSet::new();
    for tag in map_meta.tags.iter() {
        let Some(tag_id) = sqlx::query!("SELECT tag_id FROM tag_name WHERE tag_name = $1", tag)
            .fetch_optional(&state.pool)
            .await
            .context("Looking up tag name")?
        else {
            return Err(ApiErrorInner::NoSuchTag {
                tag: tag.to_owned(),
            }
            .into());
        };
        map_tags.insert(tag_id.tag_id);
    }

    // TODO configure this, and also pass to frontend
    if map_tags.len() > 7 {
        return Err(ApiErrorInner::NotAMap.into());
    }

    let map_node = gbx_rs::Node::read_from(&map_data).context("Parsing map for upload")?;
    let gbx_rs::parse::CGame::CtnChallenge(map) =
        map_node.parse().context("Parsing full map data")?
    else {
        return Err(ApiErrorInner::NotAMap.into());
    };

    let Some(map_info) = map.map_info.as_ref() else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Map info from map",
        }
        .into());
    };

    let author = nadeo::login_to_account_id(map_info.author).context("Parsing map author")?;
    if session.account_id() != author {
        return Err(ApiErrorInner::NotYourMap.into());
    }

    let Some(map_name) = map.map_name else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Map name" }.into());
    };

    let Some(thumbnail) = map.thumbnail_data else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Thumbnail" }.into());
    };

    let buffer = map_data.to_vec();

    let map_response = match sqlx::query!(
        "
            INSERT INTO map (gbx_mapuid, gbx_data, mapname, ap_author_id, created, thumbnail)
            VALUES ($1, $2, $3, $4, NOW(), $5)
            ON CONFLICT DO NOTHING
            RETURNING ap_map_id
        ",
        map_info.id,
        buffer,
        map_name,
        session.user_id(),
        thumbnail,
    )
    .fetch_optional(&state.pool)
    .await
    .context("Adding map to database")?
    {
        // you really can't get this from the function signature?
        Some(row) => MapUploadResponse {
            map_id: row.ap_map_id,
        },
        None => {
            let map_id = sqlx::query!(
                "SELECT ap_map_id FROM map WHERE gbx_mapuid = $1",
                map_info.id
            )
            .fetch_one(&state.pool)
            .await
            .context("Fetching map ID for existing map")?;
            return Err(ApiErrorInner::AlreadyUploaded {
                map_id: map_id.ap_map_id,
            }
            .into());
        }
    };

    for tag in map_tags {
        sqlx::query!(
            "INSERT INTO tag (ap_map_id, tag_id) VALUES ($1, $2)",
            map_response.map_id,
            tag
        )
        .execute(&state.pool)
        .await
        .context("Adding tag to map")?;
    }

    Ok(Json(map_response))
}
