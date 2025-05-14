use crate::nadeo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod api;
pub mod web;

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub struct UserResponse {
    pub display_name: String,
    pub account_id: String,
    pub user_id: i32,
    pub club_tag: Option<nadeo::FormattedString>,
}

#[derive(Serialize, TS)]
#[ts(export)]
struct MapContext {
    id: i32,
    gbx_uid: String,
    plain_name: String,
    name: nadeo::FormattedString,
    votes: i32,
    uploaded: String,
    created: String,
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

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct TagInfo {
    pub id: i32,
    pub name: String,
}
