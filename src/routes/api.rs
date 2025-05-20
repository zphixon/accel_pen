use super::{MapContext, TagInfo};
use crate::{
    config::CONFIG,
    error::{ApiError, ApiErrorInner, Context as _},
    nadeo::{
        self,
        api::{NadeoClubTag, NadeoUser},
        auth::{NadeoAuthSession, NadeoOauthFinishRequest, RandomStateSession},
    },
    routes::{Medals, UserResponse},
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
use std::collections::{HashMap, HashSet};
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
                "SELECT thumbnail_small FROM map_thumbnail WHERE ap_map_id = $1",
                path.map_id
            )
            .fetch_one(&state.pool)
            .await
            .context("Reading small thumbnail from database")?
            .thumbnail_small
        }
        ThumbnailSize::Large => {
            sqlx::query!(
                "SELECT thumbnail FROM map_thumbnail WHERE ap_map_id = $1",
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
    last_modified: f64,
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

    if map_meta.last_modified > i32::MAX as f64 {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "Last modified time within the next few hundred years",
        }
        .into());
    }

    let mut map_tags: HashSet<i32> = HashSet::new();
    for tag in map_meta.tags.iter() {
        let Some(tag_id) = sqlx::query!("SELECT tag_id FROM tag WHERE tag_name = $1", tag)
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

    let author_account_id =
        nadeo::login_to_account_id(map_info.author).context("Parsing map author")?;

    let Some(map_name) = map.map_name else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Map name" }.into());
    };

    let Some(thumbnail_data) = map.thumbnail_data else {
        return Err(ApiErrorInner::MissingFromMultipart { error: "Thumbnail" }.into());
    };

    if map.header_map_kind == Some(gbx_rs::MapKind::InProgress) {
        return Err(ApiErrorInner::NotValidated.into());
    }

    let Some(author_time) = map.author_time else {
        return Err(ApiErrorInner::NotValidated.into());
    };
    let Some(gold_time) = map.gold_time else {
        return Err(ApiErrorInner::NotValidated.into());
    };
    let Some(silver_time) = map.silver_time else {
        return Err(ApiErrorInner::NotValidated.into());
    };
    let Some(bronze_time) = map.bronze_time else {
        return Err(ApiErrorInner::NotValidated.into());
    };

    let ap_uploader_id = auth.user_id();
    let ap_author_id = if auth.account_id() == author_account_id {
        ap_uploader_id
    } else {
        let user = NadeoUser::get_from_account_id(&*auth, &author_account_id)
            .await
            .context("Getting author account info")?;
        let club_tag = NadeoClubTag::get(&user.account_id)
            .await
            .context("Get self club tag")?;

        match sqlx::query!(
            "SELECT ap_user_id FROM ap_user WHERE nadeo_id = $1 AND registered IS NULL",
            author_account_id
        )
        .fetch_optional(&state.pool)
        .await
        .context("Fetching AP account ID for author")?
        {
            Some(_) => return Err(ApiErrorInner::NotYourMap.into()),
            None => {
                sqlx::query!(
                    "
                INSERT INTO ap_user (nadeo_display_name, nadeo_id, nadeo_login, nadeo_club_tag)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (nadeo_id) DO UPDATE
                    SET nadeo_display_name = excluded.nadeo_display_name,
                        nadeo_club_tag = excluded.nadeo_club_tag
                RETURNING ap_user_id
            ",
                    user.display_name,
                    &user.account_id,
                    crate::nadeo::account_id_to_login(&user.account_id)?,
                    club_tag,
                )
                .fetch_one(&state.pool)
                .await
                .context("Adding user to users table")?
                .ap_user_id
            }
        }
    };

    let map_response = match sqlx::query!(
        "
            INSERT INTO map (
                gbx_mapuid, mapname, ap_author_id, ap_uploader_id, created,
                author_medal_ms, gold_medal_ms, silver_medal_ms, bronze_medal_ms 
            )
            VALUES (
                $1, $2, $3, $4, to_timestamp($5),
                $6, $7, $8, $9
            )
            ON CONFLICT DO NOTHING
            RETURNING ap_map_id
        ",
        map_info.id,
        map_name,
        ap_author_id,
        ap_uploader_id,
        map_meta.last_modified as i64,
        author_time as i32,
        gold_time as i32,
        silver_time as i32,
        bronze_time as i32
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

    sqlx::query!(
        "INSERT INTO map_data (ap_map_id, gbx_data) VALUES ($1, $2)",
        map_response.map_id,
        map_buffer
    )
    .execute(&state.pool)
    .await
    .context("Inserting map into map_data")?;

    sqlx::query!(
        "INSERT INTO map_thumbnail (ap_map_id, thumbnail, thumbnail_small) VALUES ($1, $2, $3)",
        map_response.map_id,
        thumbnail_data,
        small_thumbnail_data
    )
    .execute(&state.pool)
    .await
    .context("Inserting thumbnail data")?;

    for tag in map_tags {
        sqlx::query!(
            "INSERT INTO map_tag (ap_map_id, tag_id) VALUES ($1, $2)",
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
                    "SELECT FROM tag WHERE tag_id = $1 AND tag_name = $2 LIMIT 1",
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

            sqlx::query!("DELETE FROM map_tag WHERE ap_map_id = $1", map_id)
                .execute(&mut *transaction)
                .await
                .context("Deleting map from tag")?;

            for tag in tags.iter() {
                sqlx::query!(
                    "INSERT INTO map_tag (ap_map_id, tag_id) VALUES ($1, $2)",
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
    tagged_with: Vec<TagInfo>,
}

#[derive(Serialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub struct MapSearchResponse {
    maps: Vec<MapContext>,
}

pub async fn map_search(
    State(state): State<AppState>,
    WithRejection(Json(request), _): WithRejection<Json<MapSearchRequest>, ApiError>,
) -> Result<Json<MapSearchResponse>, ApiError> {
    let mut maps = HashMap::new();

    let mut tag_infos: Vec<TagInfo> = Vec::new();
    for tag in request.tagged_with.iter() {
        let implied_by_tag = sqlx::query!(
            "
            SELECT implied.tag_id, implied.tag_name
            FROM tag AS implyer
            JOIN tag_implies ON tag_implies.implied = implyer.tag_id
            JOIN tag AS implied ON implied.tag_id = tag_implies.implyer
            WHERE implyer.tag_id = $1
        ",
            tag.id
        )
        .fetch_all(&state.pool)
        .await
        .context("Fetching tags implied by a tag")?;
        tag_infos.push(tag.clone());
        tag_infos.extend(implied_by_tag.into_iter().map(|row| TagInfo {
            id: row.tag_id,
            name: row.tag_name,
        }));
    }

    for tag in tag_infos {
        let mut stream = sqlx::query!(
            "
            SELECT DISTINCT ON (map.ap_map_id)
                map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.ap_author_id,
                map.author_medal_ms, map.gold_medal_ms, map.silver_medal_ms, map.bronze_medal_ms,
                ap_user.nadeo_display_name, ap_user.ap_user_id, ap_user.nadeo_id, ap_user.nadeo_club_tag,
                map.created, ap_user.registered
            FROM map_tag
                JOIN map ON map_tag.ap_map_id = map.ap_map_id
                JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
            WHERE map_tag.tag_id = $1
            LIMIT 20
        ",
            tag.id,
        )
        .fetch(&state.pool);

        use futures_util::stream::TryStreamExt;
        while let Some(row) = stream
            .try_next()
            .await
            .context("Reading row from database")?
        {
            if maps.contains_key(&row.ap_map_id) {
                continue;
            }

            let tags = sqlx::query!(
                "
            SELECT DISTINCT ON (tag.tag_id) tag.tag_id, tag.tag_name
            FROM tag
            JOIN map_tag ON map_tag.tag_id = tag.tag_id
            JOIN map ON map_tag.ap_map_id = $1
        ",
                row.ap_map_id
            )
            .fetch_all(&state.pool)
            .await
            .context("Reading tags from map")?
            .into_iter()
            .map(|row| TagInfo {
                id: row.tag_id,
                name: row.tag_name,
            })
            .collect::<Vec<_>>();

            let name = crate::nadeo::FormattedString::parse(&row.mapname);

            maps.insert(
                row.ap_map_id,
                MapContext {
                    id: row.ap_map_id,
                    gbx_uid: row.gbx_mapuid,
                    plain_name: name.strip_formatting(),
                    name,
                    votes: row.votes,
                    uploaded: row
                        .uploaded
                        .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
                        .unwrap(),
                    created: row
                        .created
                        .format(&time::format_description::well_known::Iso8601::DATE_TIME_OFFSET)
                        .context("Formatting map upload time")
                        .expect("this is why i wanted to use regular Results for error handling"),
                    author: UserResponse {
                        display_name: row.nadeo_display_name,
                        account_id: row.nadeo_id,
                        user_id: row.ap_user_id,
                        club_tag: row
                            .nadeo_club_tag
                            .as_deref()
                            .map(crate::nadeo::FormattedString::parse),
                        registered: row.registered.map(super::format_time)
                    },
                    tags,
                    medals: Some(Medals {
                        author: row.author_medal_ms as u32,
                        gold: row.gold_medal_ms as u32,
                        silver: row.silver_medal_ms as u32,
                        bronze: row.bronze_medal_ms as u32,
                    }),
                },
            );
        }
    }

    Ok(Json(MapSearchResponse {
        maps: maps.into_values().collect(),
    }))
}
