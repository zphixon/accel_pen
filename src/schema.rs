// @generated automatically by Diesel CLI.

diesel::table! {
    ap_user (ap_user_id) {
        ap_user_id -> Int4,
        nadeo_display_name -> Text,
        nadeo_account_id -> Text,
        nadeo_login -> Text,
        nadeo_club_tag -> Nullable<Text>,
        site_admin -> Bool,
        registered -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    map (ap_map_id) {
        ap_map_id -> Int4,
        gbx_mapuid -> Text,
        map_name -> Text,
        votes -> Int4,
        uploaded -> Timestamptz,
        created -> Timestamptz,
        author_time -> Int4,
        gold_time -> Int4,
        silver_time -> Int4,
        bronze_time -> Int4,
    }
}

diesel::table! {
    map_data (ap_map_id) {
        ap_map_id -> Int4,
        gbx_data -> Bytea,
    }
}

diesel::table! {
    map_permission (ap_map_id, ap_user_id) {
        ap_map_id -> Int4,
        ap_user_id -> Int4,
        is_author -> Bool,
        is_uploader -> Bool,
        may_manage -> Bool,
    }
}

diesel::table! {
    map_tag (ap_map_id, tag_id) {
        ap_map_id -> Int4,
        tag_id -> Int4,
    }
}

diesel::table! {
    map_thumbnail (ap_map_id) {
        ap_map_id -> Int4,
        thumbnail -> Bytea,
        thumbnail_small -> Bytea,
    }
}

diesel::table! {
    tag (tag_id) {
        tag_id -> Int4,
        tag_name -> Text,
        tag_definition -> Nullable<Text>,
        implication -> Nullable<Int4>,
    }
}

diesel::table! {
    tag_implies (row_id) {
        row_id -> Int4,
        implication -> Int4,
        implyer -> Int4,
        implied -> Int4,
    }
}

diesel::joinable!(map_data -> map (ap_map_id));
diesel::joinable!(map_permission -> ap_user (ap_user_id));
diesel::joinable!(map_permission -> map (ap_map_id));
diesel::joinable!(map_tag -> map (ap_map_id));
diesel::joinable!(map_tag -> tag (tag_id));
diesel::joinable!(map_thumbnail -> map (ap_map_id));

diesel::allow_tables_to_appear_in_same_query!(
    ap_user,
    map,
    map_data,
    map_permission,
    map_tag,
    map_thumbnail,
    tag,
    tag_implies,
);
