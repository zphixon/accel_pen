use crate::{
    api::{TagInfo, UserResponse},
    config,
    error::Context,
    nadeo::{self, auth::NadeoAuthSession},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use serde::Serialize;
use tera::Context as TeraContext;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[ts(export)]
struct MapContext<'auth> {
    id: i32,
    gbx_uid: &'auth str,
    plain_name: String,
    name: nadeo::FormattedString,
    votes: i32,
    uploaded: String,
    author: UserResponse,
    tags: Vec<TagInfo>,
    medals: Option<Medals>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Medals {
    author: u32,
    gold: u32,
    silver: u32,
    bronze: u32,
}

fn render_error(
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
                        medals: None,
                        tags: vec![],
                    });
                }
                context.insert("my_maps", &maps_context);
            }
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error getting my maps",
                    err,
                );
            }
        }
    }

    match sqlx::query!(
        "
            SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded, map.ap_author_id,
                ap_user.nadeo_display_name, ap_user.nadeo_id, ap_user.nadeo_club_tag
            FROM map JOIN ap_user ON map.ap_author_id = ap_user.ap_user_id
            ORDER BY map.ap_map_id DESC
            LIMIT 6
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
                    medals: None,
                    tags: vec!(),
                });
            }
            context.insert("recent_maps", &recent_maps);
        }
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error getting recently uploaded maps",
                err,
            );
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

pub async fn map_page(
    State(state): State<AppState>,
    auth: Option<NadeoAuthSession>,
    Path(map_id): Path<i32>,
) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let map = match sqlx::query!(
        "
            SELECT map.ap_map_id, map.gbx_mapuid, map.mapname, map.votes, map.uploaded,
                map.ap_author_id, ap_user.nadeo_display_name, ap_user.ap_user_id, ap_user.nadeo_id,
                ap_user.nadeo_club_tag,
                map.author_medal_ms, map.gold_medal_ms, map.silver_medal_ms, map.bronze_medal_ms
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
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error reading map data",
                err,
            );
        }
    };

    let had_map = map.is_some();
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
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error reading map tags",
                    err,
                )
            }
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
                    club_tag: map
                        .nadeo_club_tag
                        .as_deref()
                        .map(nadeo::FormattedString::parse),
                },
                tags,
                medals: Some(Medals {
                    author: map.author_medal_ms as u32,
                    gold: map.gold_medal_ms as u32,
                    silver: map.silver_medal_ms as u32,
                    bronze: map.bronze_medal_ms as u32,
                }),
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
        Ok(page) => (
            if had_map {
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
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error reading map data",
                err,
            );
        }
    };

    if let Some(map) = map {
        if map.ap_author_id != auth.user_id() {
            return render_error(
                &state,
                context,
                StatusCode::UNAUTHORIZED,
                "Not allowed",
                "Cannot manage other users' maps",
            );
        }

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
            Err(err) => {
                return render_error(
                    &state,
                    context,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error reading map tags",
                    err,
                )
            }
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
                    club_tag: map
                        .nadeo_club_tag
                        .as_deref()
                        .map(nadeo::FormattedString::parse),
                },
                tags,
                medals: None,
            },
        );
    } else {
        context.insert("map", &None::<()>);
    }

    let tag_names = match sqlx::query!("SELECT tag_id, tag_name, tag_kind FROM tag_name")
        .fetch_all(&state.pool)
        .await
    {
        Ok(tag_names) => tag_names,
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error getting tag names",
                err,
            );
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
        .render("map/manage.html.tera", &context)
        .context("Rendering map page template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub async fn map_upload(State(state): State<AppState>, auth: Option<NadeoAuthSession>) -> Response {
    let mut context = config::context_with_auth_session(auth.as_ref());

    let tag_names = match sqlx::query!("SELECT tag_id, tag_name, tag_kind FROM tag_name")
        .fetch_all(&state.pool)
        .await
    {
        Ok(tag_names) => tag_names,
        Err(err) => {
            return render_error(
                &state,
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error getting tag names",
                err,
            );
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
