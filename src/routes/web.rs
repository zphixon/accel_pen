use super::{MapContext, Medals, TagInfo, UserResponse};
use crate::{
    config,
    entity::{ap_user, map, map_tag, tag},
    error::{ApiError, Context},
    nadeo::{self, auth::NadeoAuthSession},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use sea_orm::{ColumnTrait as _, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tera::Context as TeraContext;

pub fn render_error(
    state: &AppState,
    mut context: TeraContext,
    status: StatusCode,
    error: impl ToString,
    error_description: impl ToString,
) -> Response {
    context.insert("status", &u16::from(status));
    context.insert("error", &error.to_string());
    context.insert("error_description", &error_description.to_string());
    match state
        .tera
        .read()
        .unwrap()
        .render("error.html.tera", &context)
        .context("Rendering error template")
    {
        Ok(page) => (status, Html(page)).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub async fn my_fallback(
    State(state): State<AppState>,
    uri: Uri,
    auth: Option<NadeoAuthSession>,
) -> Response {
    let context = config::context_with_auth_session(auth.as_ref());
    render_error(
        &state,
        context,
        StatusCode::NOT_FOUND,
        "Not found",
        format!("Not found: {}", uri.path()),
    )
}

pub async fn index(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    if let Some(auth) = auth {
        let my_maps = match map::Entity::find()
            .filter(map::Column::Author.eq(auth.user_id()))
            .limit(20)
            .all(&state.db)
            .await
        {
            Ok(maps) => maps,
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    err,
                    "Reading my maps",
                )
            }
        };

        let mut maps_context = Vec::new();
        for map in my_maps {
            let name = nadeo::FormattedString::parse(&map.map_name);
            maps_context.push(MapContext {
                id: map.ap_map_id,
                gbx_uid: map.gbx_mapuid,
                plain_name: name.strip_formatting(),
                name,
                votes: map.votes,
                uploaded: crate::format_time(map.uploaded),
                created: crate::format_time(map.created),
                author: UserResponse {
                    display_name: auth.display_name().to_owned(),
                    account_id: auth.account_id().to_owned(),
                    user_id: auth.user_id(),
                    club_tag: auth.club_tag().map(nadeo::FormattedString::parse),
                    registered: Some(super::format_time(auth.registered())),
                },
                medals: None,
                tags: vec![],
            });
        }
        context.insert("my_maps", &maps_context);
    }

    let recent_rows = match map::Entity::find()
        .find_also_related(ap_user::Entity)
        .limit(6)
        .order_by_desc(map::Column::ApMapId)
        .all(&state.db)
        .await
    {
        Ok(recent) => recent,
        Err(error) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error getting recently uploaded maps",
                error,
            )
        }
    };

    let mut recent_maps = Vec::new();
    for (map, author) in recent_rows {
        let Some(author) = author else {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Missing author",
                "Missing author for map",
            );
        };

        let name = nadeo::FormattedString::parse(&map.map_name);
        recent_maps.push(MapContext {
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
                registered: author.registered.map(super::format_time),
            },
            medals: None,
            tags: vec![],
        });
    }
    context.insert("recent_maps", &recent_maps);

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

async fn populate_context_with_map_data(
    state: &AppState,
    map_id: i32,
    context: &mut TeraContext,
) -> Result<Option<MapContext>, ApiError> {
    let Some((map, Some(author))) = map::Entity::find_by_id(map_id)
        .find_also_related(ap_user::Entity)
        .one(&state.db)
        .await?
    else {
        return Ok(None);
    };

    let tag_models = map_tag::Entity::find()
        .filter(map_tag::Column::ApMapId.eq(map_id))
        .find_also_related(tag::Entity)
        .all(&state.db)
        .await?;

    let mut tags = Vec::new();
    for (tag_id, tag_name) in tag_models.into_iter() {
        let Some(tag_name) = tag_name else {
            return Ok(None);
        };

        tags.push(TagInfo {
            id: tag_id.tag_id,
            name: tag_name.tag_name,
        });
    }

    let name = nadeo::FormattedString::parse(&map.map_name);

    let map_context = MapContext {
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
            registered: author.registered.map(super::format_time),
        },
        tags,
        medals: Some(Medals {
            author: map.author_time as u32,
            gold: map.gold_time as u32,
            silver: map.silver_time as u32,
            bronze: map.bronze_time as u32,
        }),
    };

    context.insert("map", &map_context);
    Ok(Some(map_context))
}

pub async fn map_page(
    State(state): State<AppState>,
    auth: Option<NadeoAuthSession>,
    Path(map_id): Path<i32>,
) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let map = match populate_context_with_map_data(&state, map_id, &mut context).await {
        Ok(had_map) => had_map,
        Err(error) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                error,
                "Fetching map from database",
            )
        }
    };

    match state
        .tera
        .read()
        .unwrap()
        .render("map/page.html.tera", &context)
        .context("Rendering map page template")
    {
        Ok(page) => (
            if map.is_some() {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            },
            Html(page),
        )
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn populate_context_with_tags(
    state: &AppState,
    context: &mut TeraContext,
) -> Result<(), ApiError> {
    let tag_names = tag::Entity::find()
        .filter(tag::Column::Implication.is_not_null())
        .all(&state.db)
        .await?;

    let tag_names = tag_names
        .into_iter()
        .map(|row| TagInfo {
            id: row.tag_id,
            name: row.tag_name,
        })
        .collect::<Vec<TagInfo>>();

    context.insert("tags", &tag_names);

    Ok(())
}

pub async fn map_manage_page(
    State(state): State<AppState>,
    auth: Option<NadeoAuthSession>,
    Path(map_id): Path<i32>,
) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());
    let Some(auth) = auth else {
        return render_error(
            &state,
            context,
            StatusCode::UNAUTHORIZED,
            "Not allowed",
            "Must be logged in to manage maps",
        );
    };

    match populate_context_with_map_data(&state, map_id, &mut context).await {
        Ok(Some(map)) if map.author.user_id != auth.user_id() => {
            return render_error(
                &state,
                context,
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "This isn't your map >:(",
            );
        }

        Ok(_) => {}

        Err(error) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                error,
                "Fetching map from database",
            )
        }
    }

    if let Err(error) = populate_context_with_tags(&state, &mut context).await {
        return render_error(
            &state,
            context,
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            "Getting tags from database",
        );
    }

    match state
        .tera
        .read()
        .unwrap()
        .render("map/manage.html.tera", &context)
        .context("Rendering map page template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub async fn map_upload(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    match populate_context_with_tags(&state, &mut context).await {
        Ok(_) => {}
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                err,
                "Getting tags from database",
            );
        }
    }

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

pub async fn map_search(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    match populate_context_with_tags(&state, &mut context).await {
        Ok(_) => {}
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                err,
                "Getting tags from database",
            );
        }
    }

    match state
        .tera
        .read()
        .unwrap()
        .render("map/search.html.tera", &context)
        .context("Rendering map search template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub async fn user_page(
    State(state): State<AppState>,
    auth: Option<NadeoAuthSession>,
    Path(user_id): Path<i32>,
) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let user = match ap_user::Entity::find_by_id(user_id).one(&state.db).await {
        Ok(row) => row,
        Err(error) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                error,
                "Reading user",
            )
        }
    };

    let user_response = user.map(|row| UserResponse {
        display_name: row.nadeo_display_name,
        account_id: row.nadeo_account_id,
        user_id: row.ap_user_id,
        club_tag: row
            .nadeo_club_tag
            .as_deref()
            .map(crate::nadeo::FormattedString::parse),
        registered: row.registered.map(super::format_time),
    });

    if let Some(user) = user_response.as_ref() {
        let user_maps = match map::Entity::find()
            .filter(map::Column::Author.eq(user.user_id))
            .limit(20)
            .all(&state.db)
            .await
        {
            Ok(rows) => rows,
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error getting my maps",
                    err,
                );
            }
        };

        let mut maps_context = Vec::new();
        for map in user_maps {
            let name = nadeo::FormattedString::parse(&map.map_name);
            maps_context.push(MapContext {
                id: map.ap_map_id,
                gbx_uid: map.gbx_mapuid,
                plain_name: name.strip_formatting(),
                name,
                votes: map.votes,
                uploaded: super::format_time(map.uploaded),
                created: super::format_time(map.created),
                author: user.clone(),
                medals: None,
                tags: vec![],
            });
        }
        context.insert("user_maps", &maps_context);

        if Some(user.user_id) == auth.map(|auth| auth.user_id()) {
            // TODO
            //match sqlx::query!(
            //    "
            //        SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.created,
            //            map.ap_author_id, ap_user.nadeo_display_name, ap_user.ap_user_id, ap_user.nadeo_id,
            //            ap_user.nadeo_club_tag, ap_user.registered,
            //            map.author_medal_ms, map.gold_medal_ms, map.silver_medal_ms, map.bronze_medal_ms
            //        FROM map JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
            //        WHERE map.ap_uploader_id = $1 AND map.ap_author_id != $1
            //    ",
            //    user.user_id,
            //)
            //.fetch_all(state.db.get_postgres_connection_pool())
            //.await
            //{
            //    Ok(managed_maps) => {
            //        let mut maps_context = Vec::new();
            //        for map in managed_maps {
            //            let name = nadeo::FormattedString::parse(&map.mapname);
            //            maps_context.push(MapContext {
            //                id: map.ap_map_id,
            //                gbx_uid: map.gbx_mapuid,
            //                plain_name: name.strip_formatting(),
            //                name,
            //                votes: map.votes,
            //                uploaded: super::format_time(map
            //                    .uploaded),
            //                created: super::format_time(map
            //                    .created)
            //                    ,
            //                author: UserResponse { display_name: map.nadeo_display_name, account_id: map.nadeo_id , user_id: map.ap_user_id, club_tag: map.nadeo_club_tag.as_deref().map(nadeo::FormattedString::parse), registered: None } ,
            //                medals: None,
            //                tags: vec![],
            //            });
            //        }
            //        context.insert("managed_maps", &maps_context);
            //    }
            //    Err(err) => {
            //        return render_error(
            //            &state,
            //            context,
            //            StatusCode::INTERNAL_SERVER_ERROR,
            //            "Database error getting my maps",
            //            err,
            //        );
            //    }
            //}
        }
    }

    context.insert("page_user", &user_response);

    match state
        .tera
        .read()
        .unwrap()
        .render("user.html.tera", &context)
        .context("Rendering user template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}
