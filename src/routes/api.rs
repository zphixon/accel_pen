use super::{MapContext, TagInfo};
use crate::{
    config::CONFIG,
    error::{ApiError, ApiErrorInner, Context as _},
    nadeo::{
        self,
        auth::{NadeoAuthSession, NadeoOauthFinishRequest, RandomStateSession},
    },
    AppState,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::WithRejection;
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use std::collections::HashSet;
use tower_sessions::Session;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct OauthStartRequest {
    return_path: Option<String>,
}

pub async fn oauth_start(
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

pub async fn oauth_finish(
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

pub async fn oauth_logout(auth: NadeoAuthSession) -> Result<Redirect, ApiError> {
    auth.session().clear().await;
    Ok(Redirect::to(CONFIG.net.url.as_str()))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ThumbnailSize {
    Small,
    Large,
}

#[derive(Deserialize, Debug)]
pub struct ThumbnailPath {
    map_id: i32,
    size: ThumbnailSize,
}

pub async fn map_thumbnail(
    State(state): State<AppState>,
    WithRejection(Path(map_id), _): WithRejection<Path<i32>, ApiError>,
) -> Result<Response, ApiError> {
    map_thumbnail_inner(
        state,
        ThumbnailPath {
            map_id,
            size: ThumbnailSize::Large,
        },
    )
    .await
}

pub async fn map_thumbnail_size(
    State(state): State<AppState>,
    WithRejection(Path(path), _): WithRejection<Path<ThumbnailPath>, ApiError>,
) -> Result<Response, ApiError> {
    map_thumbnail_inner(state, path).await
}

async fn map_thumbnail_inner(state: AppState, path: ThumbnailPath) -> Result<Response, ApiError> {
    let thumbnail = match path.size {
        ThumbnailSize::Small => {
            sqlx::query!(
                "SELECT thumbnail_small FROM map WHERE ap_map_id = $1",
                path.map_id
            )
            .fetch_one(&state.pool)
            .await
            .context("Reading small thumbnail from database")?
            .thumbnail_small
        }
        ThumbnailSize::Large => {
            sqlx::query!(
                "SELECT thumbnail FROM map WHERE ap_map_id = $1",
                path.map_id
            )
            .fetch_one(&state.pool)
            .await
            .context("Reading thumbnail from database")?
            .thumbnail
        }
    };
    Ok(([(header::CONTENT_TYPE, "image/webp")], thumbnail).into_response())
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub struct MapUploadResponse {
    map_id: i32,
    map_name: String,
}

#[derive(Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
struct MapUploadMeta {
    tags: Vec<String>,
}

pub async fn map_upload(
    State(state): State<AppState>,
    auth: NadeoAuthSession,
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
    if auth.account_id() != author {
        return Err(ApiErrorInner::NotYourMap.into());
    }

    let Some(map_name) = map.map_name else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Map name" }.into());
    };

    let Some(thumbnail_data) = map.thumbnail_data else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Thumbnail" }.into());
    };

    let Some(author) = map.author_time else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Author time",
        }
        .into());
    };
    let Some(gold) = map.gold_time else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Gold time" }.into());
    };
    let Some(silver) = map.silver_time else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Silver time",
        }
        .into());
    };
    let Some(bronze) = map.bronze_time else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Bronze time",
        }
        .into());
    };

    let thumbnail = image::ImageReader::new(std::io::Cursor::new(thumbnail_data))
        .with_guessed_format()
        .context("Reading thumbnail image")?
        .decode()
        .context("Decoding thumbnail image")?;
    let thumbnail = image::imageops::flip_vertical(&thumbnail);

    let mut thumbnail_data = Vec::new();
    thumbnail
        .write_to(
            &mut std::io::Cursor::new(&mut thumbnail_data),
            image::ImageFormat::WebP,
        )
        .context("Re-encoding thumbnail")?;

    let small = image::imageops::resize(
        &thumbnail,
        thumbnail.width() / 4,
        thumbnail.height() / 4,
        image::imageops::Lanczos3,
    );
    let mut small_thumbnail_data = Vec::new();
    small
        .write_to(
            &mut std::io::Cursor::new(&mut small_thumbnail_data),
            image::ImageFormat::WebP,
        )
        .context("Re-encoding small thumbnail")?;

    let map_buffer = map_data.to_vec();

    let map_response = match sqlx::query!(
        "
            INSERT INTO map (
                gbx_mapuid, gbx_data, mapname, ap_author_id, created,
                thumbnail, thumbnail_small,
                author_medal_ms, gold_medal_ms, silver_medal_ms, bronze_medal_ms 
            )
            VALUES (
                $1, $2, $3, $4, NOW(),
                $5, $6,
                $7, $8, $9, $10
            )
            ON CONFLICT DO NOTHING
            RETURNING ap_map_id
        ",
        map_info.id,
        map_buffer,
        map_name,
        auth.user_id(),
        thumbnail_data,
        small_thumbnail_data,
        author as i32,
        gold as i32,
        silver as i32,
        bronze as i32
    )
    .fetch_optional(&state.pool)
    .await
    .context("Adding map to database")?
    {
        // you really can't get this from the function signature?
        Some(row) => MapUploadResponse {
            map_id: row.ap_map_id,
            map_name: nadeo::FormattedString::parse(map_name).strip_formatting(),
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

#[derive(Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum MapManageCommand {
    SetTags { tags: Vec<TagInfo> },
    Delete,
}

#[derive(Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub struct MapManageRequest {
    command: MapManageCommand,
}

#[derive(Serialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub struct MapManageResponse {}

pub async fn map_manage(
    State(state): State<AppState>,
    Path(map_id): Path<i32>,
    auth: NadeoAuthSession,
    WithRejection(Json(request), _): WithRejection<Json<MapManageRequest>, ApiError>,
) -> Result<Json<MapManageResponse>, ApiError> {
    let Some(_) = sqlx::query!(
        "SELECT FROM map WHERE ap_map_id = $1 AND ap_author_id = $2 LIMIT 1",
        map_id,
        auth.user_id()
    )
    .fetch_optional(&state.pool)
    .await
    .context("Checking map by user exists")?
    else {
        return Err(ApiErrorInner::MapNotFound { map_id }.into());
    };

    match &request.command {
        MapManageCommand::SetTags { tags } => {
            // 1. make sure tags exist

            for tag in tags.iter() {
                let Some(_) = sqlx::query!(
                    "SELECT FROM tag_name WHERE tag_id = $1 AND tag_name = $2 LIMIT 1",
                    tag.id,
                    tag.name
                )
                .fetch_optional(&state.pool)
                .await
                .context("Checking if tags exist")?
                else {
                    return Err(ApiErrorInner::NoSuchTag {
                        tag: tag.name.clone(),
                    }
                    .into());
                };
            }

            let mut conn = state
                .pool
                .acquire()
                .await
                .context("Acquiring connection for transaction")?;
            let mut transaction = conn.begin().await.context("Starting transaction")?;

            // 2. remove map from `tag`
            // 3. re-add map to tag with tags from `tags`

            sqlx::query!("DELETE FROM tag WHERE ap_map_id = $1", map_id)
                .execute(&mut *transaction)
                .await
                .context("Deleting map from tag")?;

            for tag in tags.iter() {
                sqlx::query!(
                    "INSERT INTO tag (ap_map_id, tag_id) VALUES ($1, $2)",
                    map_id,
                    tag.id
                )
                .execute(&mut *transaction)
                .await
                .context("Applying tag to map")?;
            }

            transaction
                .commit()
                .await
                .context("Committing transaction")?;
        }

        MapManageCommand::Delete => {
            sqlx::query!("DELETE FROM map WHERE ap_map_id = $1", map_id)
                .execute(&state.pool)
                .await
                .context("Deleting map")?;
        }
    }

    Ok(Json(MapManageResponse {}))
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct MapSearchRequest {
    tagged_with: Option<TagInfo>,
}

#[derive(Serialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub struct MapSearchResponse {
    maps: Vec<MapContext>,
}

pub async fn map_search(
    State(state): State<AppState>,
    WithRejection(Query(request), _): WithRejection<Query<MapSearchRequest>, ApiError>,
) -> Result<Json<MapSearchResponse>, ApiError> {
    Ok(Json(MapSearchResponse { maps: vec![] }))
}
