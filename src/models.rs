use crate::schema::*;
use diesel::prelude::*;

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = map)]
#[diesel(primary_key(ap_map_id))]
pub struct Map {
    pub ap_map_id: i32,
    pub gbx_mapuid: String,
    pub map_name: String,
    pub votes: i32,
    pub uploaded: time::OffsetDateTime,
    pub created: time::OffsetDateTime,
    pub author_time: i32,
    pub gold_time: i32,
    pub silver_time: i32,
    pub bronze_time: i32,
}

#[derive(Insertable)]
#[diesel(table_name = map)]
pub struct NewMap {
    pub gbx_mapuid: String,
    pub map_name: String,
    pub uploaded: time::OffsetDateTime,
    pub created: time::OffsetDateTime,
    pub author_time: i32,
    pub gold_time: i32,
    pub silver_time: i32,
    pub bronze_time: i32,
}

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = ap_user)]
#[diesel(primary_key(ap_user_id))]
pub struct User {
    pub ap_user_id: i32,
    pub nadeo_display_name: String,
    pub nadeo_account_id: String,
    pub nadeo_login: String,
    pub nadeo_club_tag: Option<String>,
    pub site_admin: bool,
    pub registered: Option<time::OffsetDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = ap_user)]
pub struct NewUser {
    pub nadeo_display_name: String,
    pub nadeo_account_id: String,
    pub nadeo_login: String,
    pub nadeo_club_tag: Option<String>,
    pub registered: Option<time::OffsetDateTime>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Insertable)]
#[diesel(table_name = map_permission)]
#[diesel(primary_key(ap_map_id, ap_user_id))]
#[diesel(belongs_to(Map, foreign_key = ap_map_id))]
#[diesel(belongs_to(User, foreign_key = ap_user_id))]
pub struct MapPermission {
    pub ap_map_id: i32,
    pub ap_user_id: i32,
    pub is_author: bool,
    pub is_uploader: bool,
    pub may_manage: bool,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = tag)]
#[diesel(primary_key(tag_id))]
#[diesel(belongs_to(TagImplies, foreign_key = implication))]
pub struct Tag {
    pub tag_id: i32,
    pub tag_name: String,
    pub implication: Option<i32>
}

#[derive(Queryable, Selectable, Identifiable, Associations, Insertable)]
#[diesel(table_name = map_tag)]
#[diesel(primary_key(ap_map_id, tag_id))]
#[diesel(belongs_to(Map, foreign_key = ap_map_id))]
#[diesel(belongs_to(Tag, foreign_key = tag_id))]
pub struct MapTag {
    pub ap_map_id: i32,
    pub tag_id: i32,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Insertable)]
#[diesel(table_name = map_data)]
#[diesel(primary_key(ap_map_id))]
#[diesel(belongs_to(Map, foreign_key = ap_map_id))]
pub struct MapData {
    pub ap_map_id: i32,
    pub gbx_data: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = map_thumbnail)]
pub struct MapThumbnail {
    pub ap_map_id: i32,
    pub thumbnail: Vec<u8>,
    pub thumbnail_small: Vec<u8>,
}

#[derive(Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = map_thumbnail)]
#[diesel(primary_key(ap_map_id))]
#[diesel(belongs_to(Map, foreign_key = ap_map_id))]
pub struct MapThumbnailLarge {
    pub ap_map_id: i32,
    pub thumbnail: Vec<u8>,
}

#[derive(Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = map_thumbnail)]
#[diesel(primary_key(ap_map_id))]
#[diesel(belongs_to(Map, foreign_key = ap_map_id))]
pub struct MapThumbnailSmall {
    pub ap_map_id: i32,
    pub thumbnail_small: Vec<u8>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tag_implies)]
pub struct TagImplies {
    pub implication: i32,
    pub implyer: i32,
    pub implied: i32,
}
