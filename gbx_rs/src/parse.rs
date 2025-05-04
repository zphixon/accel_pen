use crate::{
    cursor::{BodyCursor, CursorExt},
    Context, GbxError, GbxErrorInner, Meta,
};
use byteorder::{ReadBytesExt, LE};
use std::io::Seek;

macro_rules! parser {
    (
        $(
            $class_id:literal $variant:ident {
                $( $(#[ $ignore:meta ])? $prop:ident : $ty:ty ),* $(,)?
            } {
                $( $chunk_id:literal => $handler:expr ),* $(,)?
            }
        ),* $(,)?
    ) => {
        #[non_exhaustive]
        #[derive(Debug, Clone)]
        pub enum CGame<'node> {
            $($variant($variant<'node>),)*
        }

        impl<'node> CGame<'node> {
            pub fn supports(chunk_id: u32) -> bool {
                [$($variant::CLASS_ID,)*].contains(&chunk_id)
                $(|| $variant::supports(chunk_id))*
            }

            pub(crate) fn parse(cursor: &mut BodyCursor<'node>, class_id: u32) -> Result<CGame<'node>, GbxError> {
                match class_id {
                    $($class_id => Ok(CGame::$variant($variant::parse_full(cursor)?)),)*
                    _ => Err(GbxErrorInner::InvalidClass(class_id).into())
                }
            }

            pub(crate) fn parse_one(&mut self, cursor: &mut BodyCursor<'node>, full_chunk_id: u32) -> Result<(), GbxError> {
                match self {
                    $(CGame::$variant(inner) => $variant::parse_one(cursor, inner, full_chunk_id)?,)*
                };
                Ok(())
            }

            pub fn class_id(&self) -> u32 {
                match self {
                    $(CGame::$variant(variant) => variant.class_id(),)*
                }
            }
        }

        $(
            #[derive(Default, derivative::Derivative, Clone)]
            #[derivative(Debug)]
            #[non_exhaustive]
            pub struct $variant<'node> {
                phantom: std::marker::PhantomData<&'node ()>,
                $(
                    $(#[ $ignore ])?
                    pub $prop : $ty ,
                )*
            }

            impl<'node> Parsable<'node> for $variant<'node> {
                const CLASS_ID: u32 = $class_id;

                #[allow(unused_variables)]
                fn handle_chunk(
                    &mut self,
                    cursor: &mut BodyCursor<'node>,
                    wrapped_chunk_id: u32,
                ) -> Result<(), GbxError> {
                    match wrapped_chunk_id {
                        $($chunk_id => {
                            ($handler)(self, cursor)?;
                            Ok(())
                        })*

                        _ => {
                            return Err(GbxErrorInner::InvalidChunkForClass {
                                chunk_id: wrapped_chunk_id,
                                class_id: Self::CLASS_ID,
                            }
                            .into())
                        }
                    }
                }

                fn coerce(node: CGame<'node>) -> Result<Self, GbxError> {
                    match node {
                        CGame::$variant(variant) => Ok(variant),
                        _ => Err(GbxErrorInner::IncorrectType {
                            wanted: Self::CLASS_ID,
                            had: node.class_id(),
                        }
                        .into())
                    }
                }
            }

            impl<'node> $variant<'node> {
                pub fn supports(chunk_id: u32) -> bool {
                    [$($chunk_id,)*].contains(&chunk_id)
                }
            }
        )*
    };
}

const LAST_CHUNK_ID: u32 = 0xfacade01;

pub(crate) trait Parsable<'node>: Sized + Default {
    const CLASS_ID: u32;

    fn class_id(&self) -> u32 {
        Self::CLASS_ID
    }

    fn coerce(node: CGame<'node>) -> Result<Self, GbxError>;

    fn handle_chunk(
        &mut self,
        cursor: &mut BodyCursor<'node>,
        wrapped_chunk_id: u32,
    ) -> Result<(), GbxError>;

    fn parse_one(
        cursor: &mut BodyCursor<'node>,
        this: &mut Self,
        full_chunk_id: u32,
    ) -> Result<(), GbxError> {
        let class_id = class_wrap(full_chunk_id & 0xffff_f000);
        let wrapped_chunk_id = class_id + (full_chunk_id & 0xfff);

        tracing::trace!(
            "full chunk ID: {:08x}; wrapped chunk ID {:08x}",
            full_chunk_id,
            wrapped_chunk_id
        );
        tracing::trace!(
            "class ID: {:08x}; type class ID: {:08x}",
            class_id,
            Self::CLASS_ID
        );

        if cursor
            .peek_u32_le()
            .context("Peeking for skippable chunk")?
            == 0x53_4b_49_50
        {
            tracing::trace!("skippable");
            let _skip = cursor.read_u32::<LE>().context("Reading SKIP bytes")?;
            let chunk_data_size = cursor
                .read_u32::<LE>()
                .context("Reading skippable data size")?;

            if !CGame::supports(full_chunk_id) {
                tracing::warn!(
                    "Unsupported: class {:08x} chunk id {:08x}",
                    class_id,
                    wrapped_chunk_id
                );
                cursor
                    .seek_relative(chunk_data_size as i64)
                    .context("Skipping skippable chunk")?;
                return Ok(());
            }
        }

        this.handle_chunk(cursor, wrapped_chunk_id)
            .with_context(|| {
                format!(
                    "Handling chunk {:08x} for class {:08x}",
                    wrapped_chunk_id,
                    Self::CLASS_ID
                )
            })?;

        Ok(())
    }

    fn parse_full(cursor: &mut BodyCursor<'node>) -> Result<Self, GbxError> {
        let mut this = Self::default();
        loop {
            let full_chunk_id = cursor.read_u32::<LE>().context("Reading full chunk ID")?;
            if full_chunk_id == LAST_CHUNK_ID {
                break;
            }
            Self::parse_one(cursor, &mut this, full_chunk_id)?;
        }
        Ok(this)
    }
}

fn class_wrap(class_id: u32) -> u32 {
    match class_id {
        0x021080000 => 0x03043000, // CGameCtnChallenge (VSkipper)
        0x02108d000 => 0x03093000, // CGameCtnReplayRecord (VSkipper)
        0x024003000 => 0x03043000, // CGameCtnChallenge
        0x02400c000 => 0x0305b000, // CGameCtnChallengeParameters
        0x02401b000 => 0x03092000, // CGameCtnGhost
        0x02403a000 => 0x03059000, // CGameCtnBlockSkin
        0x02403c000 => 0x0301b000, // CGameCtnCollectorList
        0x02403f000 => 0x03093000, // CGameCtnReplayRecord
        0x024061000 => 0x03078000, // CGameCtnMediaTrack
        0x024062000 => 0x03078000, // CGameCtnMediaTrack
        0x024076000 => 0x03079000, // CGameCtnMediaClip
        0x02407e000 => 0x03093000, // CGameCtnReplayRecord
        _ => class_id,
    }
}

parser!(
    0x03043000 CtnChallenge {
        map_name: Option<&'node str>,
        vehicle_model: Option<Meta<'node>>,
        block_stock: Option<CtnCollectorList<'node>>,
        challenge_parameters: Option<CtnChallengeParameters<'node>>,
        map_kind: Option<u32>,
        map_info: Option<Meta<'node>>,
        decoration: Option<Meta<'node>>,
        size: Option<[u32; 3]>,
        bronze_time: Option<u32>,
        silver_time: Option<u32>,
        gold_time: Option<u32>,
        author_time: Option<u32>,
        cost: Option<u32>,
        header_version: Option<u32>,
        xml_data: Option<&'node str>,
        #[derivative(Debug = "ignore")]
        thumbnail_data: Option<&'node [u8]>,
        author_login: Option<&'node str>,
        author_nickname: Option<&'node str>,
        author_zone: Option<&'node str>,
        author_extra_info: Option<&'node str>,
    } {
        0x03043002 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let version = cursor.read_u8().context("Reading map info 1 version")?;

            if version <= 2 {
                this.map_info = Some(cursor.read_meta().context("Reading map info 1 map info")?);
                this.map_name = Some(cursor.read_string().context("Reading map info 1 map name")?);
            }

            let _unknown = cursor.read_u32::<LE>().context("Reading map info 1 unknown 1");

            if version >= 1 {
                this.bronze_time = Some(cursor.read_u32::<LE>().context("Reading map info 1 bronze time")?);
                this.silver_time = Some(cursor.read_u32::<LE>().context("Reading map info 1 silver time")?);
                this.gold_time = Some(cursor.read_u32::<LE>().context("Reading map info 1 gold time")?);
                this.author_time = Some(cursor.read_u32::<LE>().context("Reading map info 1 author time")?);
            }

            if version == 2 {
                let _unknown = cursor.read_u8().context("Reading map info 1 unknown 2");
            }

            if version >= 4 {
                this.cost = Some(cursor.read_u32::<LE>().context("Reading map info 1 cost (coppers (riolu LOL XD XD XD XD)")?);
            }

            if version >= 5 {
                let _is_lap_race = cursor.read_u32::<LE>().context("Reading lap race")?;
            }

            if version == 6 {
                let _is_multilap = cursor.read_u32::<LE>().context("Reading is multilap")?;
            }

            if version >= 7 {
                let _play_mode = cursor.read_u32::<LE>().context("Reading play mode")?;
            }

            if version >= 9 {
                let _unknown = cursor.read_u32::<LE>().context("Reading map info 1 unknown 3")?;
            }

            if version >= 10 {
                let _author_score = cursor.read_u32::<LE>().context("Reading map info 1 author score")?;
            }

            if version >= 11 {
                let _editor_mode = cursor.read_u32::<LE>().context("Reading editor mode")?;
            }

            if version >= 12 {
                let _unknown = cursor.read_u32::<LE>().context("Reading map info 1 unknown 4")?;
            }

            if version >= 13 {
                let _num_checkpoints = cursor.read_u32::<LE>().context("Reading num checkpoints")?;
                let _num_laps = cursor.read_u32::<LE>().context("Reading num laps")?;
            }

            Ok(())
        },

        0x03043003 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let version = cursor.read_u8().context("Reading map info 2 version")?;

            this.map_info = Some(cursor.read_meta().context("Reading map info 2 map info")?);
            this.map_name = Some(cursor.read_string().context("Reading map info 2 map name")?);
            let _map_kind = cursor.read_u8().context("Reading map info 2 map kind")?;

            if version >= 1 {
                let _unknown = cursor.read_u32::<LE>().context("Reading map info 2 unknown 1")?;
                let _password = cursor.read_string().context("Reading map info 2 password")?;
            }

            if version >= 2 {
                let _decoration = cursor.read_meta().context("Reading map info 2 decoration")?;
            }

            if version >= 3 {
                let _map_coord_origin = cursor.read_vec2().context("Reading map info 2 map coord origin")?;
            }

            if version >= 4 {
                let _map_coord_target = cursor.read_vec2().context("Reading map info 2 map coord target")?;
            }

            if version >= 5 {
                let _pack_mask = cursor.read_u128::<LE>().context("Reading map info 2 pack mask")?;
            }

            if version >= 6 {
                let _map_type = cursor.read_string().context("Reading map info 2 map type")?;
                let _map_style = cursor.read_string().context("Reading map info 2 map style")?;
            }

            if version >= 8 {
                let _lightmap_cache_uid = cursor.read_u64::<LE>().context("Reading map info 2 lightmap cache uid")?;
            }

            if version >= 9 {
                let _lightmap_version = cursor.read_u8().context("Reading map info 2 lightmap version")?;
            }

            if version >= 11 {
                let _title_id = cursor.read_lookback_string().context("Reading map info 2 title ID")?;
            }

            Ok(())
        },

        0x03043004 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.header_version = Some(cursor.read_u32::<LE>().context("Reading header version")?);
            Ok(())
        },

        0x03043005 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.xml_data = Some(cursor.read_string().context("Reading XML data")?);
            Ok(())
        },

        0x03043007 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let _version = cursor.read_u32::<LE>().context("Reading thumbnail version")?;
            let thumbnail_size = cursor.read_u32::<LE>().context("Reading thumbnail size")? as usize;
            let _thumbnail_start_tag = cursor.read_string_exact("<Thumbnail.jpg>".len()).context("Reading thumbnail start tag")?;
            let position = cursor.position() as usize;
            this.thumbnail_data = Some(&cursor.get_ref()[position..=position + thumbnail_size]);
            cursor.seek_relative(thumbnail_size as i64).context("Seeking after reading thumbnail data")?;
            let _thumbnail_end_tag = cursor.read_string_exact("</Thumbnail.jpg>".len()).context("Reading thumbnail end tag")?;
            let _comments_start_tag = cursor.read_string_exact("<Comments>".len()).context("Reading comments start tag")?;
            let _comments = cursor.read_string().context("Reading comments")?;
            let _comments_end_tag = cursor.read_string_exact("</Comments>".len()).context("Reading comments end tag")?;
            Ok(())
        },

        0x03043008 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let _version = cursor.read_u32::<LE>().context("Reading author information")?;
            let _author_version = cursor.read_u32::<LE>().context("Reading author version")?;
            this.author_login = Some(cursor.read_string().context("Reading author login")?);
            this.author_nickname = Some(cursor.read_string().context("Reading author nickname")?);
            this.author_zone = Some(cursor.read_string().context("Reading author zone")?);
            this.author_extra_info = Some(cursor.read_string().context("Reading author extra info")?);
            Ok(())
        },

        0x0304300d => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.vehicle_model = Some(cursor.read_meta().context("Reading vehicle model")?);
            Ok(())
        },

        0x03043011 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.block_stock = cursor
                .expect_node_ref::<CtnCollectorList>()
                .context("Reading block stock")?;
            this.challenge_parameters = cursor
                .expect_node_ref::<CtnChallengeParameters>()
                .context("Reading challenge parameters")?;
            this.map_kind = Some(cursor.read_u32::<LE>().context("Reading map kind")?);
            Ok(())
        },

        0x03043022 => |_this: &mut CtnChallenge, cursor: &mut BodyCursor| -> Result<(), GbxError> {
            cursor.read_u32::<LE>().context("Reading unknown")?;
            Ok(())
        },

        0x03043024 => |_this: &mut CtnChallenge, cursor: &mut BodyCursor| -> Result<(), GbxError> {
            cursor.read_file_ref().context("Reading music file reference")?;
            Ok(())
        },

        0x0304302a => |_this: &mut CtnChallenge, cursor: &mut BodyCursor| -> Result<(), GbxError> {
            cursor.read_u32::<LE>().context("Reading simple editor")?;
            Ok(())
        },

        0x03043025 => |_this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            cursor.read_f32::<LE>().context("Reading origin X")?;
            cursor.read_f32::<LE>().context("Reading origin Y")?;
            cursor.read_f32::<LE>().context("Reading target X")?;
            cursor.read_f32::<LE>().context("Reading target Y")?;
            Ok(())
        },

        0x03043049 => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            cursor.force_skip(0x03043049)?;
            Ok(())
        },

        0x0304301f => |this: &mut CtnChallenge<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("block data");
            this.map_info = Some(cursor.read_meta().context("Reading map info")?);
            this.map_name = Some(cursor.read_string().context("Reading map name")?);
            this.decoration = Some(cursor.read_meta().context("Reading decoration")?);
            this.size = Some(cursor.read_int3().context("Reading map size")?);

            let _need_unlock = cursor.read_u32::<LE>().context("Reading needs unlock")?;
            let version = cursor.read_u32::<LE>().context("Reading block data version")?;
            let num_blocks = cursor.read_u32::<LE>().context("Reading number of blocks")?;

            fn parse_block(cursor: &mut BodyCursor, version: u32, i: u32) -> Result<bool, GbxError> {
                let block_name = cursor.read_lookback_string().with_context(|| format!("Reading block {i} name"))?;
                let direction = cursor.read_u8().with_context(|| format!("Reading block {i} direction"))?;
                let position = cursor.read_byte3().with_context(|| format!("Reading block {i} position"))?;

                let flags = if version == 0 {
                    cursor.read_u16::<LE>().with_context(|| format!("Reading block {i} flags (u16)"))? as u32
                } else {
                    cursor.read_u32::<LE>().with_context(|| format!("Reading block {i} flags (u16)"))?
                };

                if flags == 0xffff_ffff {
                    Ok(false)
                } else {
                    if (flags & 0x8000) != 0 {
                        let author = cursor.read_lookback_string().with_context(|| format!("Reading block {i} author"))?;
                        let skin = cursor.expect_node_ref::<CtnBlockSkin>().with_context(|| format!("Reading block {i} skin"))?;
                    }

                    if (flags & 0x0010_0000) != 0 {
                        let waypoint_special_property = cursor
                            .expect_node_ref::<WaypointSpecialProperty>()
                            .with_context(|| format!("Reading block {i} waypoint property"))?;
                    }

                    Ok(true)
                }
            }

            let mut i = 0;
            loop {
                if parse_block(cursor, version, i)? {
                    i += 1;
                }
                if i >= num_blocks {
                    break;
                }
            }
            while (cursor.peek_u32_le().context("Peeking for more blocks")? & 0xc000_0000) > 0 {
                parse_block(cursor, version, 222222)?;
            }

            Ok(())
        },
    },

    0x0305b000 CtnChallengeParameters {
        author_score: Option<u32>,
        bronze_time: Option<u32>,
        silver_time: Option<u32>,
        gold_time: Option<u32>,
        author_time: Option<u32>,
        map_type: Option<&'node str>,
        map_style: Option<&'node str>,
        is_validated: Option<bool>,
        validation_ghost: Option<CtnGhost<'node>>,
        time_limit: Option<u32>,
        tip: Option<&'node str>,
        tip1: Option<&'node str>,
        tip2: Option<&'node str>,
        tip3: Option<&'node str>,
        tip4: Option<&'node str>,
    } {
        0x0305b001 => |this: &mut CtnChallengeParameters<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("tips");
            this.tip1 = Some(cursor.read_string().context("Reading tip 1")?);
            this.tip2 = Some(cursor.read_string().context("Reading tip 2")?);
            this.tip3 = Some(cursor.read_string().context("Reading tip 3")?);
            this.tip4 = Some(cursor.read_string().context("Reading tip 4")?);
            Ok(())
        },

        0x0305b004 => |this: &mut CtnChallengeParameters<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("medal times");
            this.bronze_time = Some(cursor.read_u32::<LE>().context("Reading bronze time")?);
            this.silver_time = Some(cursor.read_u32::<LE>().context("Reading silver time")?);
            this.gold_time = Some(cursor.read_u32::<LE>().context("Reading gold time")?);
            this.author_time = Some(cursor.read_u32::<LE>().context("Reading author time")?);
            let _unknown = cursor.read_u32::<LE>().context("Reading unknown medal time parameter")?;
            Ok(())
        },

        0x0305b008 => |this: &mut CtnChallengeParameters<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("stunt info");
            this.time_limit = Some(cursor.read_u32::<LE>().context("Reading time limit")?);
            this.author_score = Some(cursor.read_u32::<LE>().context("Reading author score")?);
            Ok(())
        },

        0x0305b00a => |this: &mut CtnChallengeParameters<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("skippable medal times");
            this.tip = Some(cursor.read_string().context("Reading skippable medal times tip")?);
            this.bronze_time = Some(cursor.read_u32::<LE>().context("Reading skippable bronze time")?);
            this.silver_time = Some(cursor.read_u32::<LE>().context("Reading skippable silver time")?);
            this.gold_time = Some(cursor.read_u32::<LE>().context("Reading skippable gold time")?);
            this.author_time = Some(cursor.read_u32::<LE>().context("Reading skippable author time")?);
            this.time_limit = Some(cursor.read_u32::<LE>().context("Reading skippable time limit")?);
            this.author_score = Some(cursor.read_u32::<LE>().context("Reading skippable author score")?);
            Ok(())
        },

        0x0305b00d => |this: &mut CtnChallengeParameters<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            tracing::trace!("stunt info");
            this.validation_ghost = cursor.expect_node_ref::<CtnGhost>().context("Reading validation ghost")?;
            Ok(())
        },
    },

    0x03092000 CtnGhost {} {},

    0x2e009000 WaypointSpecialProperty {
        order: Option<u32>,
        spawn: Option<u32>,
        tag: Option<&'node str>,
    } {
        0x2e009000 => |this: &mut WaypointSpecialProperty<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let version = cursor.read_u32::<LE>().context("Reading waypoint data version")?;

            if version == 1 {
                this.spawn = Some(cursor.read_u32::<LE>().context("Reading waypoint spawn")?);
                this.order = Some(cursor.read_u32::<LE>().context("Reading waypoint order")?);
            } else {
                this.tag = Some(cursor.read_string().context("Reading waypoint tag")?);
                this.order = Some(cursor.read_u32::<LE>().context("Reading waypoint order")?);
            }

            Ok(())
        },
    },

    0x03059000 CtnBlockSkin {
        foreground_pack_desc: Option<&'node str>,
        pack_desc: Option<&'node str>,
        parent_pack_desc: Option<&'node str>,
        text: Option<&'node str>,
    } {
        // text
        0x03059000 => |this: &mut CtnBlockSkin<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.text = Some(cursor.read_string().context("Reading block text")?);
            let _unknown = cursor.read_string().context("Reading block text unknown")?;
            Ok(())
        },

        // skin
        0x03059001 => |this: &mut CtnBlockSkin<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.text = Some(cursor.read_string().context("Reading block skin text")?);
            this.pack_desc = Some(cursor.read_file_ref().context("Reading block skin pack desc")?);
            Ok(())
        },

        // skin and parent skin
        0x03059002 => |this: &mut CtnBlockSkin<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            this.text = Some(cursor.read_string().context("Reading block skin/parent text data")?);
            this.pack_desc = Some(cursor.read_file_ref().context("Reading block skin/parent pack desc")?);
            this.parent_pack_desc = Some(cursor.read_file_ref().context("Reading block skin/parent parent pack desc")?);
            Ok(())
        },

        // secondary skin
        0x03059003 => |this: &mut CtnBlockSkin<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let _version = cursor.read_u32::<LE>().context("Reading secondary skin version")?;
            this.foreground_pack_desc = Some(cursor.read_file_ref().context("Reading secondary skin foreground pack desc")?);
            Ok(())
        },
    },

    0x0301b000 CtnCollectorList {
        block_set: Vec<Meta<'node>>,
    } {
        0x0301b000 => |this: &mut CtnCollectorList<'node>, cursor: &mut BodyCursor<'node>| -> Result<(), GbxError> {
            let len = cursor.read_u32::<LE>().context("Reading block set length")?;
            tracing::trace!("block set: {} blocks", len);
            for i in 0..len {
                this.block_set.push(
                    cursor
                        .read_meta()
                        .with_context(|| format!("Reading block set meta item {}", i))?,
                );
            }
            Ok(())
        }
    }
);
