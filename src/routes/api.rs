use super::{MapContext, TagInfo};
use crate::{
    config::CONFIG,
    entity::{ap_user, map, map_data, map_tag, map_thumbnail, tag, tag_implies},
    error::{ApiError, ApiErrorInner, Context as _},
    nadeo::{
        self,
        api::{NadeoClubTag, NadeoUser},
        auth::{NadeoAuthSession, NadeoOauthFinishRequest, RandomStateSession},
    },
    routes::UserResponse,
    AppState,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::WithRejection;
use migration::OnConflict;
use sea_orm::{
    ActiveModelTrait as _, ActiveValue::Set, ColumnTrait as _, EntityTrait as _, QueryFilter,
    QuerySelect, TransactionTrait,
};
use serde::{Deserialize, Serialize};
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
    let Some(thumbnail) = map_thumbnail::Entity::find()
        .filter(map_thumbnail::Column::ApMapId.eq(path.map_id))
        .one(&state.db)
        .await?
    else {
        return Err(ApiErrorInner::MapNotFound {
            map_id: path.map_id,
        }
        .into());
    };

    // TODO don't select both columns
    Ok((
        [(header::CONTENT_TYPE, "image/webp")],
        match path.size {
            ThumbnailSize::Small => thumbnail.thumbnail_small,
            ThumbnailSize::Large => thumbnail.thumbnail,
        },
    )
        .into_response())
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
        let Some(tag) = tag::Entity::find()
            .filter(tag::Column::TagName.eq(tag))
            .one(&state.db)
            .await?
        else {
            return Err(ApiErrorInner::NoSuchTag {
                tag: tag.to_owned(),
            }
            .into());
        };
        map_tags.insert(tag.tag_id);
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

        if ap_user::Entity::find()
            .filter(
                ap_user::Column::NadeoAccountId
                    .eq(&author_account_id)
                    .and(ap_user::Column::Registered.is_null()),
            )
            .one(&state.db)
            .await?
            .is_some()
        {
            return Err(ApiErrorInner::NotYourMap.into());
        }

        let user = ap_user::ActiveModel {
            nadeo_display_name: Set(user.display_name.clone()),
            nadeo_login: Set(nadeo::account_id_to_login(&user.account_id)?),
            nadeo_account_id: Set(user.account_id.clone()),
            nadeo_club_tag: Set(club_tag),
            ..Default::default()
        };

        ap_user::Entity::insert(user)
            .on_conflict(
                OnConflict::column(ap_user::Column::NadeoAccountId)
                    .update_columns([
                        ap_user::Column::NadeoDisplayName,
                        ap_user::Column::NadeoClubTag,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(&state.db)
            .await?
            .ap_user_id
    };

    let tx = state.db.begin().await?;

    let map_row = match (map::ActiveModel {
        author: Set(ap_author_id),
        gbx_mapuid: Set(map_info.id.to_string()),
        map_name: Set(map_name.to_string()),
        uploaded: Set(time::OffsetDateTime::now_utc()),
        created: Set(
            time::OffsetDateTime::from_unix_timestamp(map_meta.last_modified as i64).map_err(
                |_| {
                    ApiError::from(ApiErrorInner::MissingFromMultipart {
                        error: "Good timestamp for last modified",
                    })
                },
            )?,
        ),
        author_time: Set(author_time),
        gold_time: Set(gold_time),
        silver_time: Set(silver_time),
        bronze_time: Set(bronze_time),
        ..Default::default()
    }
    .insert(&state.db)
    .await)
    {
        Ok(map) => map,
        Err(err) => match map::Entity::find()
            .filter(map::Column::GbxMapuid.eq(map_info.id))
            .one(&state.db)
            .await?
        {
            Some(map) => {
                return Err(ApiErrorInner::AlreadyUploaded {
                    map_id: map.ap_map_id,
                }
                .into())
            }
            None => Err(err)?,
        },
    };
    let map_response = MapUploadResponse {
        map_id: map_row.ap_map_id,
        map_name: map_row.map_name,
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

    map_data::ActiveModel {
        ap_map_id: Set(map_response.map_id),
        gbx_data: Set(map_buffer),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    map_thumbnail::ActiveModel {
        ap_map_id: Set(map_response.map_id),
        thumbnail: Set(thumbnail_data),
        thumbnail_small: Set(small_thumbnail_data),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    for tag in map_tags {
        map_tag::ActiveModel {
            ap_map_id: Set(map_response.map_id),
            tag_id: Set(tag),
            ..Default::default()
        }
        .insert(&state.db)
        .await?;
    }

    tx.commit().await?;

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
    if map::Entity::find_by_id(map_id)
        .filter(map::Column::Author.eq(auth.user_id()))
        .one(&state.db)
        .await?
        .is_none()
    {
        return Err(ApiErrorInner::MapNotFound { map_id }.into());
    }

    match &request.command {
        MapManageCommand::SetTags { tags } => {
            // 1. make sure tags exist
            for tag in tags.iter() {
                if tag::Entity::find_by_id(tag.id)
                    .filter(tag::Column::TagName.eq(&tag.name))
                    .one(&state.db)
                    .await?
                    .is_none()
                {
                    return Err(ApiErrorInner::NoSuchTag {
                        tag: tag.name.clone(),
                    }
                    .into());
                };
            }

            let tx = state.db.begin().await?;

            // 2. remove map from `tag`
            map_tag::Entity::delete_many()
                .filter(map_tag::Column::ApMapId.eq(map_id))
                .exec(&state.db)
                .await?;

            // 3. re-add map to tag with tags from `tags`
            map_tag::Entity::insert_many(tags.into_iter().map(|tag| map_tag::ActiveModel {
                ap_map_id: Set(map_id),
                tag_id: Set(tag.id),
                ..Default::default()
            }))
            .exec(&state.db)
            .await?;

            tx.commit().await?;
        }

        MapManageCommand::Delete => {
            map::Entity::delete_by_id(map_id).exec(&state.db).await?;
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
    let mut maps = HashMap::<i32, MapContext>::new();

    let mut tag_infos: Vec<TagInfo> = Vec::new();
    for tag in request.tagged_with.iter() {
        tag_infos.push(tag.clone());
        let implications = tag_implies::Entity::find()
            .filter(tag_implies::Column::Implyer.eq(tag.id))
            .all(&state.db)
            .await?;
        for implication in implications {
            let Some(implied) = tag::Entity::find_by_id(implication.implied)
                .one(&state.db)
                .await?
            else {
                return Err(ApiErrorInner::NoSuchTag {
                    tag: tag.name.clone(),
                }
                .into());
            };
            tag_infos.push(TagInfo {
                id: implied.tag_id,
                name: implied.tag_name,
            });
        }
    }

    for tag in tag_infos {
        for (map, author, _) in map::Entity::find()
            .distinct()
            .find_also_related(ap_user::Entity)
            .find_also_related(map_tag::Entity)
            .filter(map_tag::Column::TagId.eq(tag.id))
            .limit(20)
            .all(&state.db)
            .await?
        {
            let Some(author) = author else {
                // hmmmmm
                return Err(ApiErrorInner::MapNotFound {
                    map_id: map.ap_map_id,
                }
                .into());
            };

            let tags = map_tag::Entity::find()
                .find_also_related(tag::Entity)
                .filter(map_tag::Column::ApMapId.eq(map.ap_map_id))
                .all(&state.db)
                .await?
                .into_iter()
                .flat_map(|(_, tag)| tag)
                .map(|tag| TagInfo {
                    id: tag.tag_id,
                    name: tag.tag_name,
                }).collect();

            let name = nadeo::FormattedString::parse(&map.map_name);
            maps.insert(
                map.ap_map_id,
                MapContext {
                    id: map.ap_map_id,
                    gbx_uid: map.gbx_mapuid,
                    plain_name: name.strip_formatting(),
                    name,
                    votes: map.votes,
                    uploaded: crate::format_time(map.uploaded),
                    created: crate::format_time(map.created),
                    author: UserResponse {
                        display_name: author.nadeo_display_name,
                        account_id: author.nadeo_account_id,
                        user_id: author.ap_user_id,
                        club_tag: author
                            .nadeo_club_tag
                            .as_deref()
                            .map(nadeo::FormattedString::parse),
                        registered: author.registered.map(crate::format_time),
                    },
                    tags,
                    medals: None,
                },
            );
        }
    }

    Ok(Json(MapSearchResponse {
        maps: maps.into_values().collect(),
    }))
}
