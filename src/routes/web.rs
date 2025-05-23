use super::{MapContext, Medals, TagInfo, UserResponse};
use crate::{
    config,
    error::{ApiError, ApiErrorInner, Context},
    nadeo::{self, auth::NadeoAuthSession},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Serialize;
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
    let mut conn = match state.db.get().await {
        Ok(conn) => conn,
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Getting database connection",
                err,
            );
        }
    };

    if let Some(auth) = auth {
        let user: crate::models::User = match crate::schema::ap_user::dsl::ap_user
            .find(auth.user_id())
            .select(crate::models::User::as_select())
            .get_result(&mut conn)
            .await
        {
            Ok(user) => user,
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Reading user self",
                    err,
                )
            }
        };

        let my_maps: Vec<crate::models::Map> =
            match crate::models::MapPermission::belonging_to(&user)
                .inner_join(crate::schema::map::table)
                .select(crate::models::Map::as_select())
                .limit(20)
                .get_results(&mut conn)
                .await
            {
                Ok(my_maps) => my_maps,
                Err(err) => {
                    return render_error(
                        &state,
                        context,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Reading my maps",
                        err,
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

    let recent_rows = match crate::schema::map::dsl::map
        .select(crate::models::Map::as_select())
        .order_by(crate::schema::map::dsl::ap_map_id.desc())
        .limit(6)
        .get_results(&mut conn)
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
    for recent in recent_rows {
        let author: crate::models::User = match crate::models::MapPermission::belonging_to(&recent)
            .inner_join(crate::schema::ap_user::table)
            .select(crate::models::User::as_select())
            .filter(crate::schema::map_permission::dsl::is_author.eq(true))
            .get_result(&mut conn)
            .await
            .optional()
        {
            Ok(Some(user)) => user,
            Ok(None) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Missing author",
                    "Missing author for map",
                )
            }
            Err(err) => {
                // TODO less repetition
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error getting recently uploaded maps",
                    err,
                );
            }
        };

        let name = nadeo::FormattedString::parse(&recent.map_name);
        recent_maps.push(MapContext {
            id: recent.ap_map_id,
            gbx_uid: recent.gbx_mapuid,
            plain_name: name.strip_formatting(),
            name,
            votes: recent.votes,
            uploaded: crate::format_time(recent.uploaded),
            created: crate::format_time(recent.created),
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

    let tags = crate::models::MapTag::belonging_to(&map)
        .inner_join(crate::schema::tag::table)
        .select(crate::models::Tag::as_select())
        .get_results(&mut conn)
        .await?
        .into_iter()
        .map(|tag| TagInfo {
            id: tag.tag_id,
            name: tag.tag_name,
        })
        .collect();

    let users: Vec<(crate::models::MapPermission, crate::models::User)> =
        match crate::models::MapPermission::belonging_to(&map)
            .inner_join(crate::schema::ap_user::table)
            .select((
                crate::models::MapPermission::as_select(),
                crate::models::User::as_select(),
            ))
            .get_results(&mut conn)
            .await
        {
            Ok(users) => users,
            Err(err) => {
                return Err(err.into());
            }
        };

    let Some((author_perms, author)) = users.iter().find(|(user, _)| user.is_author) else {
        return Err(ApiErrorInner::MissingFromMultipart {
            error: "gheawjhoewjifoewaojfie",
        }
        .into());
    };

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
            display_name: author.nadeo_display_name.clone(),
            account_id: author.nadeo_account_id.clone(),
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

    if !author_perms.is_uploader {
        let Some((_, uploader)) = users.iter().find(|(user, _)| user.is_uploader) else {
            return Err(ApiErrorInner::MissingFromMultipart {
                error: "Missing uploader jafiowejoifeawiofjio",
            }
            .into());
        };

        context.insert(
            "uploader",
            &UserResponse {
                display_name: uploader.nadeo_display_name.clone(),
                account_id: uploader.nadeo_account_id.clone(),
                user_id: uploader.ap_user_id,
                club_tag: uploader
                    .nadeo_club_tag
                    .as_deref()
                    .map(nadeo::FormattedString::parse),
                registered: uploader.registered.map(super::format_time),
            },
        );
    }

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
    let mut conn = state.db.get().await?;
    let tag_infos = crate::schema::tag::dsl::tag
        .select(crate::models::Tag::as_select())
        .filter(crate::schema::tag::dsl::implication.is_not_null())
        .get_results(&mut conn)
        .await?
        .into_iter()
        .map(|tag| TagInfo {
            id: tag.tag_id,
            name: tag.tag_name,
        })
        .collect::<Vec<_>>();
    context.insert("tags", &tag_infos);
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

    let mut conn = match state.db.get().await {
        Ok(conn) => conn,
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Getting database connection",
                err,
            );
        }
    };

    let map = match crate::schema::map::dsl::map
        .select(crate::models::Map::as_select())
        .find(map_id)
        .get_result(&mut conn)
        .await
        .optional()
    {
        Ok(Some(map)) => map,
        Ok(None) => {
            return render_error(
                &state,
                context,
                StatusCode::NOT_FOUND,
                "Map not found",
                "Map not found",
            )
        }
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Getting map hmmmmmmmm i hate this thing",
                err,
            );
        }
    };

    let users: Vec<(crate::models::User, crate::models::MapPermission)> =
        match crate::models::MapPermission::belonging_to(&map)
            .inner_join(crate::schema::ap_user::table)
            .select((
                crate::models::User::as_select(),
                crate::models::MapPermission::as_select(),
            ))
            .get_results(&mut conn)
            .await
        {
            Ok(users) => users,
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Getting map hmmmmmmmm i hate this thing",
                    err,
                );
            }
        };

    if !users
        .iter()
        .find(|(user, perms)| user.ap_user_id == auth.user_id() && perms.may_manage)
        .is_some()
    {
        return render_error(
            &state,
            context,
            StatusCode::UNAUTHORIZED,
            "Not allowed",
            "You are not allowed to manage this map",
        );
    }

    if let Err(error) = populate_context_with_map_data(&state, map_id, &mut context).await {
        return render_error(
            &state,
            context,
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            "Fetching map from database",
        );
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

    let mut conn = match state.db.get().await {
        Ok(conn) => conn,
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Getting database connection",
                err,
            );
        }
    };

    let user: crate::models::User = match crate::schema::ap_user::dsl::ap_user
        .select(crate::models::User::as_select())
        .find(user_id)
        .get_result(&mut conn)
        .await
    {
        Ok(user) => user,
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

    let authored_maps: Vec<crate::models::Map> =
        match crate::models::MapPermission::belonging_to(&user)
            .inner_join(crate::schema::map::table)
            .select(crate::models::Map::as_select())
            .filter(crate::schema::map_permission::dsl::is_author.eq(true))
            .get_results(&mut conn)
            .await
        {
            Ok(maps) => maps,
            Err(error) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error,
                    "Getting user's maps",
                )
            }
        };

    let user_response = UserResponse {
        display_name: user.nadeo_display_name.clone(),
        account_id: user.nadeo_account_id.clone(),
        user_id: user.ap_user_id.clone(),
        club_tag: user
            .nadeo_club_tag
            .as_deref()
            .map(crate::nadeo::FormattedString::parse),
        registered: user.registered.map(super::format_time),
    };

    let mut maps_context = Vec::new();
    for map in authored_maps.iter() {
        let name = nadeo::FormattedString::parse(&map.map_name);
        maps_context.push(MapContext {
            id: map.ap_map_id,
            gbx_uid: map.gbx_mapuid.clone(),
            plain_name: name.strip_formatting(),
            name,
            votes: map.votes,
            uploaded: super::format_time(map.uploaded),
            created: super::format_time(map.created),
            author: user_response.clone(),
            medals: None,
            tags: vec![],
        });
    }
    context.insert("user_maps", &maps_context);

    if Some(user_response.user_id) == auth.map(|auth| auth.user_id()) {
        let managed_maps: Vec<crate::models::Map> =
            match crate::models::MapPermission::belonging_to(&user)
                .inner_join(crate::schema::map::table)
                .select(crate::models::Map::as_select())
                .filter(
                    crate::schema::map_permission::dsl::may_manage
                        .eq(true)
                        .and(crate::schema::map_permission::dsl::is_author.eq(false)),
                )
                .get_results(&mut conn)
                .await
            {
                Ok(managed) => managed,
                Err(error) => {
                    return render_error(
                        &state,
                        context,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        error,
                        "Getting maps managed  by user",
                    )
                }
            };

        let mut map_contexts = Vec::<MapContext>::new();
        for map in managed_maps {
            let author: crate::models::User = match crate::models::MapPermission::belonging_to(&map)
                .inner_join(crate::schema::ap_user::table)
                .select(crate::models::User::as_select())
                .filter(crate::schema::map_permission::dsl::is_author.eq(true))
                .get_result(&mut conn)
                .await
                .optional()
            {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return render_error(
                        &state,
                        context,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Missing author",
                        "Missing author for map",
                    )
                }
                Err(err) => {
                    // TODO less repetition
                    return render_error(
                        &state,
                        context,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Database error getting recently uploaded maps",
                        err,
                    );
                }
            };

            let name = nadeo::FormattedString::parse(&map.map_name);
            map_contexts.push(MapContext {
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
                tags: vec![],
                medals: None,
            });
        }

        context.insert("managed_maps", &map_contexts);

        #[derive(Serialize)]
        struct ManagedByOtherUser {
            map: MapContext,
            user: UserResponse,
        }

        let mut managed_by_others = Vec::new();
        for map in authored_maps.iter() {
            let users: Vec<(crate::models::MapPermission, crate::models::User)> =
                match crate::models::MapPermission::belonging_to(map)
                    .inner_join(crate::schema::ap_user::table)
                    .select((
                        crate::models::MapPermission::as_select(),
                        crate::models::User::as_select(),
                    ))
                    .get_results(&mut conn)
                    .await
                {
                    Ok(users) => users,
                    Err(err) => {
                        // TODO less repetition
                        return render_error(
                            &state,
                            context,
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Getting maps user has participated in",
                            err,
                        );
                    }
                };

            if let Some((_, other_user)) = users
                .iter()
                .find(|(perms, user)| user.ap_user_id != user_response.user_id && perms.may_manage)
            {
                let name = nadeo::FormattedString::parse(&map.map_name);
                managed_by_others.push(ManagedByOtherUser {
                    user: UserResponse {
                        display_name: other_user.nadeo_display_name.clone(),
                        account_id: other_user.nadeo_account_id.clone(),
                        user_id: other_user.ap_user_id,
                        club_tag: other_user
                            .nadeo_club_tag
                            .as_deref()
                            .map(nadeo::FormattedString::parse),
                        registered: other_user.registered.map(crate::format_time),
                    },
                    map: MapContext {
                        id: map.ap_map_id,
                        gbx_uid: map.gbx_mapuid.clone(),
                        plain_name: name.strip_formatting(),
                        name,
                        votes: map.votes,
                        uploaded: crate::format_time(map.uploaded),
                        created: crate::format_time(map.created),
                        author: user_response.clone(),
                        tags: vec![],
                        medals: None,
                    },
                });
            }
        }
        context.insert("managed_by_others", &managed_by_others);
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
