use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ApUser::Table)
                    .col(pk_auto(ApUser::ApUserId))
                    .col(string(ApUser::NadeoDisplayName))
                    .col(string(ApUser::NadeoAccountId).unique_key())
                    .col(string(ApUser::NadeoLogin).unique_key())
                    .col(string_null(ApUser::NadeoClubTag))
                    .col(boolean(ApUser::SiteAdmin).default(false))
                    .col(timestamp_with_time_zone_null(ApUser::Registered))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Map::Table)
                    .col(pk_auto(Map::ApMapId))
                    .col(integer(Map::Author))
                    .col(string(Map::GbxMapuid).unique_key())
                    .col(string(Map::MapName))
                    .col(integer(Map::Votes).default(1))
                    .col(timestamp_with_time_zone(Map::Uploaded))
                    .col(timestamp_with_time_zone(Map::Created))
                    .col(integer(Map::AuthorTime))
                    .col(integer(Map::GoldTime))
                    .col(integer(Map::SilverTime))
                    .col(integer(Map::BronzeTime))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Map::Table, Map::Author)
                            .to(ApUser::Table, ApUser::ApUserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MapThumbnail::Table)
                    .col(pk_auto(MapThumbnail::Dummy))
                    .col(integer(MapThumbnail::ApMapId))
                    .col(binary(MapThumbnail::Thumbnail))
                    .col(binary(MapThumbnail::ThumbnailSmall))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_map_thumbnail_ap_map_id_map")
                            .from(MapThumbnail::Table, MapThumbnail::ApMapId)
                            .to(Map::Table, Map::ApMapId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MapData::Table)
                    .col(pk_auto(MapData::Dummy))
                    .col(integer(MapData::ApMapId))
                    .col(binary(MapData::GbxData))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_map_data_ap_map_id_map")
                            .from(MapData::Table, MapData::ApMapId)
                            .to(Map::Table, Map::ApMapId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Tag::Table)
                    .col(pk_auto(Tag::TagId))
                    .col(text(Tag::TagName))
                    .col(text_null(Tag::TagDefinition))
                    .col(integer_null(Tag::Implication))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TagImplies::Table)
                    .col(pk_auto(TagImplies::Dummy))
                    .col(integer(TagImplies::Implication))
                    .col(integer(TagImplies::Implyer))
                    .col(integer(TagImplies::Implied))
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagImplies::Table, TagImplies::Implyer)
                            .to(Tag::Table, Tag::TagId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagImplies::Table, TagImplies::Implied)
                            .to(Tag::Table, Tag::TagId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MapTag::Table)
                    .col(pk_auto(MapTag::Dummy))
                    .col(integer(MapTag::ApMapId))
                    .col(integer(MapTag::TagId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(MapTag::Table, MapTag::ApMapId)
                            .to(Map::Table, Map::ApMapId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MapTag::Table, MapTag::TagId)
                            .to(Tag::Table, Tag::TagId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApUser::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Map::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MapThumbnail::Table)
                    .cascade()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(MapData::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tag::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TagImplies::Table).cascade().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MapTag::Table).cascade().to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ApUser {
    Table,
    ApUserId,
    NadeoDisplayName,
    NadeoAccountId,
    NadeoLogin,
    NadeoClubTag,
    SiteAdmin,
    Registered,
}

#[derive(DeriveIden)]
enum Map {
    Table,
    ApMapId,
    Author,
    GbxMapuid,
    MapName,
    Votes,
    Uploaded,
    Created,
    AuthorTime,
    GoldTime,
    SilverTime,
    BronzeTime,
}

#[derive(DeriveIden)]
enum MapThumbnail {
    Table,
    Dummy,
    ApMapId,
    Thumbnail,
    ThumbnailSmall,
}

#[derive(DeriveIden)]
enum MapData {
    Table,
    Dummy,
    ApMapId,
    GbxData,
}

#[derive(DeriveIden)]
pub enum Tag {
    Table,
    TagId,
    TagName,
    TagDefinition,
    Implication,
}

#[derive(DeriveIden)]
pub enum TagImplies {
    Table,
    Dummy,
    Implication,
    Implyer,
    Implied,
}

#[derive(DeriveIden)]
pub enum MapTag {
    Table,
    Dummy,
    ApMapId,
    TagId,
}
