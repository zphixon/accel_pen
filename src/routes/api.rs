use super::{MapContext, Permission, TagInfo};
use crate::{
    config::CONFIG,
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
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
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
    let mut conn = state.db.get().await?;

    let Some(map) = crate::schema::map::dsl::map
        .select(crate::models::Map::as_select())
        .find(path.map_id)
        .get_result(&mut conn)
        .await
        .optional()?
    else {
        return Err(ApiErrorInner::MapNotFound {
            map_id: path.map_id,
        }
        .into());
    };

    let thumbnail_data: Vec<u8> = match path.size {
        ThumbnailSize::Small => {
            crate::models::MapThumbnailSmall::belonging_to(&map)
                .select(crate::models::MapThumbnailSmall::as_select())
                .get_result(&mut conn)
                .await?
                .thumbnail_small
        }
        ThumbnailSize::Large => {
            crate::models::MapThumbnailLarge::belonging_to(&map)
                .select(crate::models::MapThumbnailLarge::as_select())
                .get_result(&mut conn)
                .await?
                .thumbnail
        }
    };

    // TODO don't select both columns
    Ok(([(header::CONTENT_TYPE, "image/webp")], thumbnail_data).into_response())
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
        return Err(ApiErrorInner::LastModifiedTimeTooLarge.into());
    }

    let mut conn = state.db.get().await?;

    let mut map_tags: HashSet<i32> = HashSet::new();
    for tag in map_meta.tags.iter() {
        let Some(tag): Option<crate::models::Tag> = crate::schema::tag::dsl::tag
            .select(crate::models::Tag::as_select())
            .filter(crate::schema::tag::dsl::tag_name.eq(tag))
            .get_result(&mut conn)
            .await
            .optional()?
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
        return Err(ApiErrorInner::InvalidMap {
            error: "Map info from map",
        }
        .into());
    };

    let author_account_id =
        nadeo::login_to_account_id(map_info.author).context("Parsing map author")?;

    let Some(map_name) = map.map_name else {
        return Err(ApiErrorInner::InvalidMap {
            error: "Missing map name",
        }
        .into());
    };

    let Some(thumbnail_data) = map.thumbnail_data else {
        return Err(ApiErrorInner::InvalidMap {
            error: "Missing thumbnail",
        }
        .into());
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
        let maybe_user: Option<crate::models::User> = crate::schema::ap_user::dsl::ap_user
            .select(crate::models::User::as_select())
            .filter(crate::schema::ap_user::dsl::nadeo_account_id.eq(&author_account_id))
            .get_result(&mut conn)
            .await
            .optional()?;

        if maybe_user.and_then(|user| user.registered).is_some() {
            return Err(ApiErrorInner::NotYourMapUpload.into());
        }

        let user = NadeoUser::get_from_account_id(&*auth, &author_account_id)
            .await
            .context("Getting new author account info for upload")?;
        let club_tag = NadeoClubTag::get(&user.account_id)
            .await
            .context("Get new author club tag for upload")?;

        let new_user: crate::models::User = diesel::insert_into(crate::schema::ap_user::table)
            .values(crate::models::NewUser {
                nadeo_display_name: user.display_name.clone(),
                nadeo_login: crate::nadeo::account_id_to_login(&user.account_id)?,
                nadeo_account_id: user.account_id.clone(),
                nadeo_club_tag: club_tag.clone(),
                registered: None,
            })
            .returning(crate::models::User::as_returning())
            .get_result(&mut conn)
            .await?;

        new_user.ap_user_id
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

    let new_map = match diesel::insert_into(crate::schema::map::table)
        .values(crate::models::NewMap {
            gbx_mapuid: map_info.id.to_string(),
            map_name: map_name.to_string(),
            uploaded: time::OffsetDateTime::now_utc(),
            created: time::OffsetDateTime::from_unix_timestamp(map_meta.last_modified as i64)?,
            author_time,
            gold_time,
            silver_time,
            bronze_time,
        })
        .returning(crate::models::Map::as_returning())
        .get_result(&mut conn)
        .await
    {
        Ok(new_map) => new_map,
        Err(err) => {
            if let Some(exists) = crate::schema::map::dsl::map
                .select(crate::models::Map::as_select())
                .filter(crate::schema::map::dsl::gbx_mapuid.eq(map_info.id))
                .get_result(&mut conn)
                .await
                .optional()?
            {
                return Err(ApiErrorInner::AlreadyUploaded {
                    map_id: exists.ap_map_id,
                }
                .into());
            } else {
                return Err(err.into());
            }
        }
    };

    diesel::insert_into(crate::schema::map_data::table)
        .values(crate::models::MapData {
            ap_map_id: new_map.ap_map_id,
            gbx_data: map_buffer,
        })
        .execute(&mut conn)
        .await?;

    diesel::insert_into(crate::schema::map_thumbnail::table)
        .values(crate::models::MapThumbnail {
            ap_map_id: new_map.ap_map_id,
            thumbnail: thumbnail_data,
            thumbnail_small: small_thumbnail_data,
        })
        .execute(&mut conn)
        .await?;

    for tag in map_tags {
        diesel::insert_into(crate::schema::map_tag::table)
            .values(crate::models::MapTag {
                ap_map_id: new_map.ap_map_id,
                tag_id: tag,
            })
            .execute(&mut conn)
            .await?;
    }

    if ap_author_id == ap_uploader_id {
        diesel::insert_into(crate::schema::map_permission::table)
            .values(crate::models::MapPermission {
                ap_map_id: new_map.ap_map_id,
                ap_user_id: ap_author_id,
                is_author: true,
                is_uploader: true,
                may_manage: true,
                may_grant: true,
                other: None,
            })
            .execute(&mut conn)
            .await?;
    } else {
        diesel::insert_into(crate::schema::map_permission::table)
            .values(crate::models::MapPermission {
                ap_map_id: new_map.ap_map_id,
                ap_user_id: ap_author_id,
                is_author: true,
                is_uploader: false,
                may_manage: true,
                may_grant: true,
                other: None,
            })
            .execute(&mut conn)
            .await?;
        diesel::insert_into(crate::schema::map_permission::table)
            .values(crate::models::MapPermission {
                ap_map_id: new_map.ap_map_id,
                ap_user_id: ap_uploader_id,
                is_author: false,
                is_uploader: true,
                may_manage: true,
                may_grant: false,
                other: None,
            })
            .execute(&mut conn)
            .await?;
    }

    Ok(Json(MapUploadResponse {
        map_id: new_map.ap_map_id,
        map_name: nadeo::FormattedString::parse(&new_map.map_name).strip_formatting(),
    }))
}

#[derive(Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum MapManageCommand {
    SetTags { tags: Vec<TagInfo> },
    SetPermissions { permissions: Vec<Permission> },
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
    let mut conn = state.db.get().await?;
    let Some(map) = crate::schema::map::dsl::map
        .select(crate::models::Map::as_select())
        .find(map_id)
        .get_result(&mut conn)
        .await
        .optional()?
    else {
        return Err(ApiErrorInner::MapNotFound { map_id }.into());
    };

    let user: crate::models::MapPermission = crate::models::MapPermission::belonging_to(&map)
        .inner_join(crate::schema::ap_user::table)
        .filter(crate::schema::map_permission::dsl::ap_user_id.eq(auth.user_id()))
        .select(crate::models::MapPermission::as_select())
        .get_result(&mut conn)
        .await?;

    if !user.is_author || !user.may_manage {
        return Err(ApiErrorInner::NotYourMapManage.into());
    }

    match &request.command {
        MapManageCommand::SetTags { tags } => {
            // 1. make sure tags exist
            for tag in tags.iter() {
                if crate::schema::tag::dsl::tag
                    .find(tag.id)
                    .filter(crate::schema::tag::dsl::tag_name.eq(&tag.name))
                    .select(crate::models::Tag::as_select())
                    .get_result(&mut conn)
                    .await
                    .optional()?
                    .is_none()
                {
                    return Err(ApiErrorInner::NoSuchTag {
                        tag: tag.name.clone(),
                    }
                    .into());
                };
            }

            // 2. remove map from `tag`
            diesel::delete(crate::schema::map_tag::table)
                .filter(crate::schema::map_tag::dsl::ap_map_id.eq(map.ap_map_id))
                .execute(&mut conn)
                .await?;

            // 3. re-add map to tag with tags from `tags`
            for tag in tags {
                diesel::insert_into(crate::schema::map_tag::table)
                    .values(crate::models::MapTag {
                        ap_map_id: map.ap_map_id,
                        tag_id: tag.id,
                    })
                    .execute(&mut conn)
                    .await?;
            }
        }

        MapManageCommand::Delete => {
            diesel::delete(crate::schema::map::table)
                .filter(crate::schema::map::dsl::ap_map_id.eq(map.ap_map_id))
                .execute(&mut conn)
                .await?;
        }

        MapManageCommand::SetPermissions { permissions } => {
            if !user.may_grant {
                return Err(ApiErrorInner::NotYourMapGrant.into());
            }

            for perm in permissions {
                if perm.user_id == auth.user_id() {
                    return Err(ApiErrorInner::CannotUsurpAuthor.into());
                }

                diesel::update(crate::schema::map_permission::table)
                    .filter(
                        crate::schema::map_permission::dsl::ap_user_id
                            .eq(perm.user_id)
                            .and(crate::schema::map_permission::dsl::ap_map_id.eq(map_id)),
                    )
                    .set(crate::models::UpdateMapPermission {
                        may_manage: perm.may_manage,
                        may_grant: perm.may_grant,
                    })
                    .execute(&mut conn)
                    .await?;
            }
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
    let mut conn = state.db.get().await?;

    let mut tag_infos: Vec<crate::models::Tag> = Vec::new();
    for tag in request.tagged_with.iter() {
        tag_infos.push(
            // TODO error on unknown tag
            crate::schema::tag::dsl::tag
                .find(tag.id)
                .filter(crate::schema::tag::dsl::tag_name.eq(&tag.name))
                .select(crate::models::Tag::as_select())
                .get_result(&mut conn)
                .await?,
        );

        // TODO better schema that avoids n+1?
        let implications = crate::schema::tag_implies::dsl::tag_implies
            .select(crate::models::TagImplies::as_select())
            .filter(crate::schema::tag_implies::dsl::implyer.eq(tag.id))
            .get_results(&mut conn)
            .await?;

        for implication in implications {
            let tag: crate::models::Tag = crate::schema::tag::dsl::tag
                .select(crate::models::Tag::as_select())
                .find(implication.implied)
                .get_result(&mut conn)
                .await?;
            tag_infos.push(tag);
        }
    }

    let mut maps = HashMap::<i32, MapContext>::new();
    for tag in tag_infos {
        let maps_tagged: Vec<crate::models::Map> = crate::models::MapTag::belonging_to(&tag)
            .inner_join(crate::schema::map::table)
            .select(crate::models::Map::as_select())
            .get_results(&mut conn)
            .await?;

        for map in maps_tagged {
            let tags = crate::models::MapTag::belonging_to(&map)
                .inner_join(crate::schema::tag::table)
                .select(crate::models::Tag::as_select())
                .get_results(&mut conn)
                .await?;

            let users: Vec<(crate::models::MapPermission, crate::models::User)> =
                crate::models::MapPermission::belonging_to(&map)
                    .inner_join(crate::schema::ap_user::table)
                    .select((
                        crate::models::MapPermission::as_select(),
                        crate::models::User::as_select(),
                    ))
                    .get_results(&mut conn)
                    .await?;

            let name = nadeo::FormattedString::parse(&map.map_name);

            let Some((_, author)) = users.iter().find(|(user, _)| user.is_author) else {
                return Err(ApiErrorInner::MissingAuthor {
                    map_id: map.ap_map_id,
                }
                .into());
            };

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
                        display_name: author.nadeo_display_name.clone(),
                        account_id: author.nadeo_account_id.clone(),
                        user_id: author.ap_user_id,
                        club_tag: author
                            .nadeo_club_tag
                            .as_deref()
                            .map(nadeo::FormattedString::parse),
                        registered: author.registered.map(crate::format_time),
                    },
                    tags: tags
                        .into_iter()
                        .map(|tag| TagInfo {
                            id: tag.tag_id,
                            name: tag.tag_name,
                        })
                        .collect(),
                    medals: Some(super::Medals {
                        author: map.author_time as u32,
                        gold: map.gold_time as u32,
                        silver: map.silver_time as u32,
                        bronze: map.bronze_time as u32,
                    }),
                },
            );
        }
    }

    Ok(Json(MapSearchResponse {
        maps: maps.into_values().collect(),
    }))
}
