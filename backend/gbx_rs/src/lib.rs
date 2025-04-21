use byteorder::{ReadBytesExt, LE};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    io::{Cursor, Read, Seek},
    ops::{Deref, DerefMut},
};

trait Context<T> {
    fn context<C>(self, context: C) -> Result<T, GbxError>
    where
        C: std::fmt::Display + Send + Sync + 'static;

    fn with_context<F, C>(self, context_fn: F) -> Result<T, GbxError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display + Send + Sync + 'static;
}

impl<T, E: Into<GbxError>> Context<T> for Result<T, E> {
    fn context<C>(self, context: C) -> Result<T, GbxError>
    where
        C: std::fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(err) => Err(GbxError::Context {
                context: context.to_string(),
                inner: Box::new(err.into()),
            }),
        }
    }

    fn with_context<F, C>(self, context_fn: F) -> Result<T, GbxError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(err) => Err(GbxError::Context {
                context: context_fn().to_string(),
                inner: Box::new(err.into()),
            }),
        }
    }
}

#[derive(Debug)]
pub enum GbxError {
    Root(GbxErrorInner),
    Context {
        context: String,
        inner: Box<GbxError>,
    },
}

impl<T: Into<GbxErrorInner>> From<T> for GbxError {
    fn from(value: T) -> Self {
        GbxError::Root(value.into())
    }
}

impl Display for GbxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GbxError::Root(inner) => write!(f, "{}", inner),
            GbxError::Context { context, inner } => {
                Display::fmt(inner, f)?;
                write!(f, "\n  {}", context)
            }
        }
    }
}

impl Deref for GbxError {
    type Target = GbxErrorInner;

    fn deref(&self) -> &Self::Target {
        match self {
            GbxError::Root(inner) => inner,
            GbxError::Context { inner, .. } => {
                let box_ref = Box::as_ref(inner);
                <GbxError as Deref>::deref(box_ref)
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum GbxErrorInner {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Not a GBX file")]
    NotGbx,

    #[error("GBX version {0} not supported")]
    VersionNotSupported(u16),

    #[error("No header chunks")]
    NoHeaderChunks,

    #[error("Could not decompress")]
    Lzo(#[from] lzokay_native::Error),

    #[error("Not compressed - not sure what to do with uncompressed gbx data yet")]
    NotCompressed,

    #[error("Invalid byte format {0}")]
    InvalidByteFormat(u8),

    #[error("Invalid compression state {0}")]
    InvalidCompressionState(u8),

    #[error("No such chunk with ID {0}")]
    NoSuchChunk(u32),

    #[error("Wanted to parse class ID {wanted:08x}, had class ID {had:08x} instead")]
    IncorrectType { wanted: u32, had: u32 },

    #[error("Invalid UTF-8")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("Invalid lookback string, file may be corrupted")]
    InvalidLookbackString,

    #[error("TODO: {0}")]
    TODO(&'static str),

    #[error("Invalid chunk {chunk_id:08x} for class {class_id:08x}")]
    InvalidChunkForClass { chunk_id: u32, class_id: u32 },

    #[error("Invalid string from {start:08x} to {end:08x}")]
    InvalidString { start: usize, end: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ByteFormat {
    Text,
    Binary,
}

impl TryFrom<u8> for ByteFormat {
    type Error = GbxErrorInner;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            b'B' => Ok(ByteFormat::Binary),
            b'T' => Ok(ByteFormat::Text),
            _ => Err(GbxErrorInner::InvalidByteFormat(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Compression {
    Compressed,
    Uncompressed,
}

impl TryFrom<u8> for Compression {
    type Error = GbxErrorInner;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            b'C' => Ok(Compression::Compressed),
            b'U' => Ok(Compression::Uncompressed),
            _ => Err(GbxErrorInner::InvalidCompressionState(value)),
        }
    }
}

#[derive(Debug)]
struct Header {
    version: u16,
    byte_format: ByteFormat,
    body_compression: Compression,
    class_id: u32,
    chunks: Vec<HeaderChunk>,
    num_nodes: u32,
    num_external_nodes: u32,
}

#[derive(Debug)]
struct HeaderChunk {
    id: u32,
    size: u32,
    heavy: bool,
    data_start: u64,
}

fn parse_header<B: AsRef<[u8]>>(cursor: &mut Cursor<B>) -> Result<Header, GbxError> {
    let mut magic = [0u8; 3];
    cursor.read_exact(&mut magic).context("Reading magic")?;
    if magic != *b"GBX" {
        tracing::debug!("magic {:?}", magic);
        return Err(GbxErrorInner::NotGbx.into());
    }

    let version = cursor.read_u16::<LE>().context("Reading version")?;
    if version < 3 {
        return Err(GbxErrorInner::VersionNotSupported(version).into());
    }
    tracing::debug!("version {}", version);

    let byte_format = cursor.read_u8().context("Reading byte format")?;
    tracing::debug!("byte format {:?}", byte_format as char);
    let byte_format = byte_format.try_into().context("Parsing byte format")?;

    let _ref_table_compression = cursor.read_u8().context("Reading ref table compression")?;
    tracing::debug!("ref table compression {:?}", _ref_table_compression as char);
    let _ref_table_compression: Compression = _ref_table_compression
        .try_into()
        .context("Parsing ref table compression")?;

    let body_compression = cursor.read_u8().context("Reading body compression")?;
    tracing::debug!("body compression {:?}", body_compression as char);
    let body_compression = body_compression
        .try_into()
        .context("Parsing body compression")?;

    if version >= 4 {
        let _unknown = cursor.read_u8().context("Reading unknown 1")?;
        tracing::debug!("unknown {}", _unknown);
    }

    let class_id = cursor.read_u32::<LE>().context("Reading class id")?;
    tracing::debug!("class id 0x{:08x}", class_id);

    if version >= 6 {
        let _user_data_size = cursor.read_u32::<LE>().context("Reading user data size")?;
        tracing::debug!("user data size {}", _user_data_size);
    }

    let num_header_chunks = cursor
        .read_u32::<LE>()
        .context("Reading num header chunks")?;
    if num_header_chunks == 0 {
        return Err(GbxErrorInner::NoHeaderChunks.into());
    }

    tracing::debug!("num header chunks {}", num_header_chunks);

    let mut chunks = Vec::new();
    for i in 0..num_header_chunks {
        tracing::debug!("chunk {}", i);

        let id = cursor
            .read_u32::<LE>()
            .with_context(|| format!("Reading chunk {i} ID"))?
            & 0x0fff;
        tracing::debug!("  chunk id 0x{:08x}", id);

        let chunk_size_heavy = cursor
            .read_u32::<LE>()
            .with_context(|| format!("Reading chunk {i} size"))?;
        let size = chunk_size_heavy & !0x8000_0000;
        tracing::debug!("  chunk size {}", size);

        let heavy = (chunk_size_heavy & 0x8000_0000) != 0;
        tracing::debug!("  heavy {}", heavy);

        chunks.push(HeaderChunk {
            id,
            size,
            heavy,
            data_start: 0,
        });
    }

    for chunk in chunks.iter_mut() {
        chunk.data_start = cursor.position();
        cursor
            .seek_relative(chunk.size as i64)
            .with_context(|| format!("Reading chunk {} data", chunk.id))?;
    }

    let num_nodes = cursor.read_u32::<LE>().context("Reading num nodes")?;
    tracing::debug!("num nodes {}", num_nodes);
    let num_external_nodes = cursor
        .read_u32::<LE>()
        .context("Reading num external nodes")?;
    tracing::debug!("num external nodes {}", num_external_nodes);

    Ok(Header {
        version,
        byte_format,
        body_compression,
        class_id,
        chunks,
        num_nodes,
        num_external_nodes,
    })
}

pub struct Node {
    header: Header,
    //body_start: u64,
    body: Vec<u8>,
}

impl Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("header", &self.header)
            .field("body", &"...")
            .finish()
    }
}

impl Node {
    pub fn read_from<B: AsRef<[u8]>>(data: B) -> Result<Node, GbxError> {
        let mut cursor = Cursor::new(data);
        let header = parse_header(&mut cursor).context("Parsing header")?;
        if header.body_compression != Compression::Compressed {
            return Err(GbxErrorInner::NotCompressed.into());
        }

        let uncompressed_size = cursor
            .read_u32::<LE>()
            .context("Reading uncompressed size")?;
        tracing::debug!("uncompressed size {}", uncompressed_size);
        let compressed_size = cursor.read_u32::<LE>().context("Reading compressed size")?;
        tracing::debug!("compressed size {}", compressed_size);
        //let body_start = cursor.position();
        //tracing::debug!("body start {}", body_start);

        Ok(Node {
            header,
            //body_start,
            body: lzokay_native::decompress(&mut cursor, None).context("Decompressing body")?,
        })
    }

    pub fn to<'this, N: FromNode<'this>>(&'this self) -> Result<N, GbxError> {
        if self.header.class_id != N::CLASS_ID {
            Err(GbxErrorInner::IncorrectType {
                had: self.header.class_id,
                wanted: N::CLASS_ID,
            }
            .into())
        } else {
            N::from_node(BodyCursor::new(Cursor::new(&self.body)))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Meta<'node> {
    pub id: &'node str,
    pub collection: &'node str,
    pub author: &'node str,
}

trait CursorExt {
    fn peek_u32_le(&mut self) -> Result<u32, GbxError>;
}

impl CursorExt for Cursor<&'_ [u8]> {
    fn peek_u32_le(&mut self) -> Result<u32, GbxError> {
        let value = self.read_u32::<LE>()?;
        self.seek_relative(-4)?;
        Ok(value)
    }
}

#[derive(Clone)]
struct BodyCursor<'node> {
    inner: Cursor<&'node [u8]>,
    lookback_version: Option<u32>,
    strings: Vec<&'node str>,
    nodes: HashMap<i32, *mut ()>,
}

impl Debug for BodyCursor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BodyCursor")
            .field("lookback_version", &self.lookback_version)
            .field("strings", &self.strings)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl<'node> Deref for BodyCursor<'node> {
    type Target = Cursor<&'node [u8]>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for BodyCursor<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'node> BodyCursor<'node> {
    fn new(cursor: Cursor<&'node [u8]>) -> Self {
        BodyCursor {
            inner: cursor,
            lookback_version: None,
            strings: Vec::new(),
            nodes: HashMap::new(),
        }
    }

    fn read_string(&mut self) -> Result<&'node str, GbxError> {
        let count = self.read_u32::<LE>().context("Reading string length")?;
        let start = self.position() as usize;
        let end = start + count as usize;

        if start >= self.get_ref().len() || end >= self.get_ref().len() {
            return Err(GbxErrorInner::InvalidString { start, end }.into());
        }

        let str =
            std::str::from_utf8(&self.get_ref()[start..end]).context("Reading string data")?;
        self.seek_relative(count as i64)
            .context("Seeking after reading string")?;
        Ok(str)
    }

    fn read_meta(&mut self) -> Result<Meta<'node>, GbxError> {
        Ok(Meta {
            id: self.read_lookback_string().context("Reading meta ID")?,
            collection: self
                .read_lookback_string()
                .context("Reading meta collection")?,
            author: self.read_lookback_string().context("Reading meta author")?,
        })
    }

    fn read_lookback_string(&mut self) -> Result<&'node str, GbxError> {
        if self.lookback_version.is_none() {
            self.lookback_version =
                Some(self.read_u32::<LE>().context("Reading lookback version")?);
        }

        let index = self.read_u32::<LE>().context("Reading lookback index")?;
        if index == 0xffff_ffffu32 {
            return Ok("");
        }
        let index = index as i64;

        if (index & 0x3fff) == 0 && (index >> 30 == 1 || index >> 30 == -2) {
            let str = self
                .read_string()
                .context("Reading first occurrence of lookback string")?;
            tracing::trace!("new string {:?}", str);
            self.strings.push(str);
            return Ok(str);
        }

        if (index & 0x3fff) == 0x3fff {
            match index >> 30 {
                2 => return Ok("Unassigned"),
                3 => return Ok(""),
                _ => return Err(GbxErrorInner::InvalidLookbackString.into()),
            }
        }

        if (index >> 30) == 0 {
            return Ok("TODO: collections");
        }

        if self.strings.len() > ((index & 0x3fff) - 1) as usize {
            Ok(self.strings[((index & 0x3fff) - 1) as usize])
        } else {
            Ok("")
        }
    }

    fn read_node_reference<N: FromNode<'node>>(&mut self) -> Result<Option<N>, GbxError> {
        let index = self
            .read_i32::<LE>()
            .context("Reading node reference index")?;
        //tracing::trace!("node reference index {index}");

        if index == -1 {
            return Ok(None);
        }

        if let Some(&scary_pointer) = self.nodes.get(&index) {
            // this is not how it works
            // this is not how it works
            // this is not how it works
            // this is not how it works
            // this is not how it works
            // this is not how it works
            // this is not how it works
            let b = unsafe { Box::from_raw(scary_pointer as *mut N) };
            if b.class_id() != N::CLASS_ID {
                return Err(GbxErrorInner::IncorrectType {
                    wanted: N::CLASS_ID,
                    had: b.class_id(),
                }
                .into());
            }
            let new_b = b.clone();
            let _ = Box::into_raw(b);
            return Ok(Some(*new_b));
        }

        if index >= 0 {
            let class_id = self
                .read_u32::<LE>()
                .context("Reading node reference class ID")?;

            if class_id != N::CLASS_ID {
                Err(GbxErrorInner::IncorrectType {
                    had: class_id,
                    wanted: N::CLASS_ID,
                }
                .into())
            } else {
                let position = self.position() as usize;
                let mut node =
                    N::from_node(BodyCursor::new(Cursor::new(&self.get_ref()[position..])))?;
                tracing::trace!("reading {:08x}", class_id);
                node.read_full().context("Reading node reference")?;
                let forward = node.cursor().position() as i64;
                tracing::trace!("read {} bytes of {:08x}", forward, class_id);
                self.seek_relative(forward)
                    .context("Seeking after read node reference")?;

                self.nodes
                    .insert(index, Box::into_raw(Box::new(node.clone())) as *mut ());

                Ok(Some(node))
            }
        } else {
            unreachable!("index < 0: {index}");
        }
    }

    fn read_int3(&mut self) -> Result<[u32; 3], GbxError> {
        Ok([
            self.read_u32::<LE>()?,
            self.read_u32::<LE>()?,
            self.read_u32::<LE>()?,
        ])
    }

    fn read_byte3(&mut self) -> Result<[u8; 3], GbxError> {
        Ok([self.read_u8()?, self.read_u8()?, self.read_u8()?])
    }

    fn read_file_reference(&mut self) -> Result<&'node str, GbxError> {
        let version = self.read_u8().context("Reading file reference version")?;
        if version >= 3 {
            self.seek_relative(32)
                .context("Seeking past file reference checksum")?; // checksum
        }
        let file_path = self.read_string().context("Reading file reference path")?;
        if (file_path.len() > 0 && version >= 1) || version >= 3 {
            let _locator_url = self
                .read_string()
                .context("Reading file reference locator URL")?;
        }
        Ok(file_path)
    }
}

mod sealed {
    use crate::{BodyCursor, GbxError};

    pub(super) trait FromNodeInner<'node> {
        fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError>
        where
            Self: Sized;

        fn cursor(&self) -> &BodyCursor<'node>;

        fn cursor_mut(&mut self) -> &mut BodyCursor<'node>;

        fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError>;
    }
}

#[allow(private_bounds)]
pub trait FromNode<'node>: sealed::FromNodeInner<'node> + Clone {
    const CLASS_ID: u32;

    fn class_id(&self) -> u32 {
        Self::CLASS_ID
    }

    fn read_full(&mut self) -> Result<(), GbxError> {
        loop {
            let full_chunk_id = self
                .cursor_mut()
                .read_u32::<LE>()
                .context("Reading full chunk ID")?;
            if full_chunk_id == 0xfacade01 {
                break;
            }

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

            if self
                .cursor_mut()
                .peek_u32_le()
                .context("Peeking for skippable chunk")?
                == 0x53_4b_49_50
            {
                tracing::trace!("skippable");
                let _skip = self
                    .cursor_mut()
                    .read_u32::<LE>()
                    .context("Reading SKIP bytes")?;
                let chunk_data_size = self
                    .cursor_mut()
                    .read_u32::<LE>()
                    .context("Reading skippable data size")?;
                tracing::warn!("TODO: check if skippable chunks supported");
                tracing::trace!("skipping {} bytes", chunk_data_size);
                self.cursor_mut()
                    .seek_relative(chunk_data_size as i64)
                    .context("Skipping skippable chunk")?;
                continue;
            }

            self.handle_chunk(wrapped_chunk_id).with_context(|| {
                format!(
                    "Handling chunk {:08x} for class {:08x}",
                    wrapped_chunk_id,
                    Self::CLASS_ID
                )
            })?;
        }

        Ok(())
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

#[non_exhaustive]
#[derive(Clone)]
pub struct CGameCtnChallenge<'node> {
    cursor: BodyCursor<'node>,
    pub map_name: Option<&'node str>,
    pub vehicle_model: Option<Meta<'node>>,
    pub block_stock: Option<CGameCtnCollectorList<'node>>,
    pub challenge_parameters: Option<CGameCtnChallengeParameters<'node>>,
    pub map_kind: Option<u32>,
    pub map_info: Option<Meta<'node>>,
    pub decoration: Option<Meta<'node>>,
    pub size: Option<[u32; 3]>,
}

impl<'node> sealed::FromNodeInner<'node> for CGameCtnChallenge<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError> {
        Ok(CGameCtnChallenge {
            cursor,
            map_name: None,
            vehicle_model: None,
            block_stock: None,
            challenge_parameters: None,
            map_kind: None,
            map_info: None,
            decoration: None,
            size: None,
        })
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            Self::MAP_INFORMATION1 => {
                tracing::trace!("map information 1");
            }

            Self::VEHICLE_MODEL => {
                tracing::trace!("vehicle model");
                self.vehicle_model =
                    Some(self.cursor.read_meta().context("Reading vehicle model")?);
                tracing::trace!("{:#?}", self.vehicle_model);
            }

            Self::MAP_INFORMATION3 => {
                tracing::trace!("map information 3");

                self.block_stock = self
                    .cursor
                    .read_node_reference::<CGameCtnCollectorList>()
                    .context("Reading block stock")?;
                tracing::trace!("block stock: {:?}", self.block_stock);

                self.challenge_parameters = self
                    .cursor
                    .read_node_reference::<CGameCtnChallengeParameters>()
                    .context("Reading challenge parameters")?;
                tracing::trace!("challenge parameters: {:#?}", self.challenge_parameters);

                self.map_kind = Some(self.cursor.read_u32::<LE>().context("Reading map kind")?);
            }

            Self::BLOCK_DATA => {
                tracing::trace!("block data");
                self.map_info = Some(self.cursor.read_meta().context("Reading map info")?);
                self.map_name = Some(self.cursor.read_string().context("Reading map name")?);
                self.decoration = Some(self.cursor.read_meta().context("Reading decoration")?);
                self.size = Some(self.cursor.read_int3().context("Reading map size")?);

                let _need_unlock = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading needs unlock")?;

                let version = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading block data version")?;

                let num_blocks = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading number of blocks")?;

                let mut i = 0;
                let mut why = 0;
                let mut read_block = || -> Result<(u32, bool), GbxError> {
                    let mut is_normal = true;

                    let block_name = self
                        .cursor
                        .read_lookback_string()
                        .with_context(|| format!("Reading block {why} name"))?;

                    let direction = self
                        .cursor
                        .read_u8()
                        .with_context(|| format!("Reading block {why} direction"))?;

                    let position = self
                        .cursor
                        .read_byte3()
                        .with_context(|| format!("Reading block {why} position"))?;

                    let flags = if version == 0 {
                        self.cursor
                            .read_u16::<LE>()
                            .with_context(|| format!("Reading block {why} flags (u16)"))?
                            as u32
                    } else {
                        self.cursor
                            .read_u32::<LE>()
                            .with_context(|| format!("Reading block {why} flags (u32)"))?
                    };

                    if flags == 0xffff_ffff {
                        is_normal = false;
                    } else {
                        if (flags & 0x8000) != 0 {
                            let author = self
                                .cursor
                                .read_lookback_string()
                                .with_context(|| format!("Reading block {why} author"))?;

                            let skin = self
                                .cursor
                                .read_node_reference::<CGameCtnBlockSkin>()
                                .with_context(|| format!("Reading block {why} skin"))?;
                        }

                        if (flags & 0x10_0000) != 0 {
                            // waypointsaveme
                            let waypoint_special_property = self
                                .cursor
                                .read_node_reference::<CGameWaypointSpecialProperty>()
                                .with_context(|| {
                                    format!("Reading block {why} waypoint property")
                                })?;
                        }
                    }

                    if is_normal {
                        i += 1;
                    }
                    why += 1;

                    Ok((
                        self.cursor
                            .peek_u32_le()
                            .context("Peeking for new blocks")?,
                        i < num_blocks,
                    ))
                };

                tracing::trace!("{} blocks", num_blocks);
                let mut peek = 0;
                loop {
                    let (peek1, go) = read_block()?;
                    peek = peek1;
                    if !go {
                        break;
                    }
                }
                while (peek & 0xc000_0000) > 0 {
                    let (peek1, _) = read_block()?;
                    peek = peek1;
                }
            }

            Self::UNKNOWN => {
                self.cursor.read_u32::<LE>().context("Reading unknown")?;
            }

            Self::MUSIC => {
                self.cursor
                    .read_file_reference()
                    .context("Reading music file reference")?;
            }

            Self::ORIGIN_TARGET => {
                self.cursor.read_f32::<LE>().context("Reading origin X")?;
                self.cursor.read_f32::<LE>().context("Reading origin Y")?;
                self.cursor.read_f32::<LE>().context("Reading target X")?;
                self.cursor.read_f32::<LE>().context("Reading target Y")?;
            }

            Self::SIMPLE_EDITOR => {
                self.cursor
                    .read_u32::<LE>()
                    .context("Reading simple editor")?;
            }

            Self::MEDIATRACKER => loop {
                let next_chunk_id_maybe =
                    self.cursor.peek_u32_le().context("Force skipping chunk")?;
                if (next_chunk_id_maybe & 0xffff_ff00) == (Self::MEDIATRACKER & 0xffff_ff00) {
                    tracing::warn!("Force skipped chunk {:08x}", Self::MEDIATRACKER);
                    break;
                }
                if next_chunk_id_maybe == 0xfacade01
                    && self.cursor.peek_u32_le().is_err_and(|err| match &*err {
                        GbxErrorInner::Io(io) => {
                            matches!(io.kind(), std::io::ErrorKind::UnexpectedEof)
                        }
                        _ => false,
                    })
                {
                    tracing::warn!("Force skipped chunk {:08x}", Self::MEDIATRACKER);
                    tracing::warn!("Reached end of file early");
                    break;
                }
                let _skipped_byte = self
                    .cursor
                    .read_u8()
                    .context("Force skipping media tracker")?;
            },

            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }

        Ok(())
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }
}

impl<'node> FromNode<'node> for CGameCtnChallenge<'node> {
    const CLASS_ID: u32 = 0x0304_3000;
}

#[allow(unused)]
impl<'node> CGameCtnChallenge<'node> {
    const MAP_INFORMATION1: u32 = 0x0304_3002;
    const MAP_INFORMATION2: u32 = 0x0304_3003;
    const HEADER_VERSION: u32 = 0x0304_3004;
    const XML_DATA: u32 = 0x0304_3005;
    const THUMBNAIL_DATA: u32 = 0x0304_3007;
    const AUTHOR_INFO: u32 = 0x0304_3008;
    const VEHICLE_MODEL: u32 = 0x0304_300d;
    // const BLOCK_DATA: u32 = 0x0304_300f;
    const MAP_INFORMATION3: u32 = 0x0304_3011;
    const BLOCK_DATA: u32 = 0x0304_301f;
    const UNKNOWN: u32 = 0x0304_3022;
    const MUSIC: u32 = 0x0304_3024;
    const ORIGIN_TARGET: u32 = 0x0304_3025;
    const SIMPLE_EDITOR: u32 = 0x0304_302a;
    const MEDIATRACKER: u32 = 0x0304_3049;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CGameCtnCollectorList<'node> {
    cursor: BodyCursor<'node>,
    pub block_set: Vec<Meta<'node>>,
}

impl<'node> sealed::FromNodeInner<'node> for CGameCtnCollectorList<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError> {
        Ok(CGameCtnCollectorList {
            cursor,
            block_set: Vec::new(),
        })
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            Self::BLOCK_SET => {
                let len = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading block set length")?;
                tracing::trace!("block set: {} blocks", len);
                for i in 0..len {
                    self.block_set.push(
                        self.cursor
                            .read_meta()
                            .with_context(|| format!("Reading block set meta item {}", i))?,
                    );
                }
            }

            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }

        Ok(())
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }
}

impl<'node> FromNode<'node> for CGameCtnCollectorList<'node> {
    const CLASS_ID: u32 = 0x0301_b000;
}

impl<'node> CGameCtnCollectorList<'node> {
    const BLOCK_SET: u32 = 0x0301_b000;
}

#[non_exhaustive]
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct CGameCtnChallengeParameters<'node> {
    cursor: BodyCursor<'node>,
    pub author_score: Option<u32>,
    pub bronze_time: Option<u32>,
    pub silver_time: Option<u32>,
    pub gold_time: Option<u32>,
    pub author_time: Option<u32>,
    pub map_type: Option<&'node str>,
    pub map_style: Option<&'node str>,
    pub is_validated: Option<bool>,
    pub validation_ghost: Option<CGameCtnGhost<'node>>,
    pub time_limit: Option<u32>,
    pub tip: Option<&'node str>,
    pub tip1: Option<&'node str>,
    pub tip2: Option<&'node str>,
    pub tip3: Option<&'node str>,
    pub tip4: Option<&'node str>,
}

impl<'node> sealed::FromNodeInner<'node> for CGameCtnChallengeParameters<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError>
    where
        Self: Sized,
    {
        Ok(CGameCtnChallengeParameters {
            cursor,
            author_score: None,
            bronze_time: None,
            silver_time: None,
            gold_time: None,
            author_time: None,
            map_type: None,
            map_style: None,
            is_validated: None,
            validation_ghost: None,
            time_limit: None,
            tip: None,
            tip1: None,
            tip2: None,
            tip3: None,
            tip4: None,
        })
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            Self::TIPS => {
                tracing::trace!("tips");
                self.tip1 = Some(self.cursor.read_string().context("Reading tip 1")?);
                self.tip2 = Some(self.cursor.read_string().context("Reading tip 2")?);
                self.tip3 = Some(self.cursor.read_string().context("Reading tip 3")?);
                self.tip4 = Some(self.cursor.read_string().context("Reading tip 4")?);
            }

            Self::MEDAL_TIMES => {
                tracing::trace!("medal times");
                self.bronze_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading bronze time")?,
                );
                self.silver_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading silver time")?,
                );
                self.gold_time = Some(self.cursor.read_u32::<LE>().context("Reading gold time")?);
                self.author_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading author time")?,
                );
                let _unknown = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading unknown medal time parameter")?;
            }

            Self::STUNTS => {
                tracing::trace!("stunt info");
                self.time_limit = Some(self.cursor.read_u32::<LE>().context("Reading time limit")?);
                self.author_score = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading author score")?,
                );
            }

            Self::MEDAL_TIMES_SKIPPABLE => {
                tracing::trace!("skippable medal times");
                self.tip = Some(
                    self.cursor
                        .read_string()
                        .context("Reading skippable medal times tip")?,
                );
                self.bronze_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable bronze time")?,
                );
                self.silver_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable silver time")?,
                );
                self.gold_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable gold time")?,
                );
                self.author_time = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable author time")?,
                );
                self.time_limit = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable time limit")?,
                );
                self.author_score = Some(
                    self.cursor
                        .read_u32::<LE>()
                        .context("Reading skippable author score")?,
                );
            }

            Self::RACE_VALIDATION_GHOST => {
                tracing::trace!("validation ghost");
                self.validation_ghost = self
                    .cursor
                    .read_node_reference::<CGameCtnGhost>()
                    .context("Reading validation ghost")?;
            }

            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }

        Ok(())
    }
}

impl<'node> FromNode<'node> for CGameCtnChallengeParameters<'node> {
    const CLASS_ID: u32 = 0x0305_b000;
}

#[allow(unused)]
impl CGameCtnChallengeParameters<'_> {
    const UNKNOWN1: u32 = 0x0305_b000;
    const TIPS: u32 = 0x0305_b001;
    const UNKNOWN2: u32 = 0x0305_b002;
    const UNKNOWN3: u32 = 0x0305_b003;
    const MEDAL_TIMES: u32 = 0x0305_b004;
    const UNKNOWN4: u32 = 0x0305_b005;
    const ITEMS: u32 = 0x0305_b006;
    const UNKNOWN5: u32 = 0x0305_b007;
    const STUNTS: u32 = 0x0305_b008;
    const MEDAL_TIMES_SKIPPABLE: u32 = 0x0305_b00a; // skippable??
    const RACE_VALIDATION_GHOST: u32 = 0x0305_b00d;
    const MAP_TYPE_SKIPPABLE: u32 = 0x0305_b00e;
}

#[derive(Debug, Clone)]
pub struct CGameCtnGhost<'node> {
    cursor: BodyCursor<'node>,
}

impl<'node> sealed::FromNodeInner<'node> for CGameCtnGhost<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError>
    where
        Self: Sized,
    {
        Ok(CGameCtnGhost { cursor })
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }
    }
}

impl<'node> FromNode<'node> for CGameCtnGhost<'node> {
    const CLASS_ID: u32 = 0x0309_2000;
}

#[allow(unused)]
impl CGameCtnGhost<'_> {
    const BASIC_INFO_SKIPPABLE: u32 = 0x0309_2000;
    const DRIVER_DATA1: u32 = 0x0309_2003;
    const CHECKPOINTS1_SKIPPABLE: u32 = 0x0309_2004;
    const RACE_TIME_SKIPPABLE: u32 = 0x0309_2005;
    const DRIVER_DATA2: u32 = 0x0309_2006;
    const LIGHT_TRAIL_COLOR_OLD: u32 = 0x0309_2007;
    const RESPAWNS_SKIPPABLE: u32 = 0x0309_2008;
    const LIGHT_TRAIL_COLOR: u32 = 0x0303_2009;
    const STUNT_SCORE_SKIPPABLE: u32 = 0x0309_200a;
    const CHECKPOINTS2_SKIPPABLE: u32 = 0x0309_200b;
    const UNKNOWN1: u32 = 0x0309_200c;
    const DRIVER_DATA3: u32 = 0x0309_200d;
    const GHOST_UUID: u32 = 0x0309_200e;
    const GHOST_LOGIN: u32 = 0x0309_200f;
    const VALIDATION_MAP_UID: u32 = 0x0309_2010;
    const VALIDATION: u32 = 0x0309_2011;
    const UNKNOWN2: u32 = 0x0309_2012;
    const UNKNOWN3_SKIPPABLE: u32 = 0x0309_2013;
    const UNKNOWN4_SKIPPABLE: u32 = 0x0309_2014;
    const GHOST_NICKNAME: u32 = 0x0309_2015;
    const GHOST_METADATA_SKIPPABLE: u32 = 0x0309_2017;
    const PLAYER_MODEL: u32 = 0x0309_2018;
    const VALIDATION2: u32 = 0x0309_2019;
    const UNKNOWN5_SKIPPABLE: u32 = 0x0309_201a;
    const UNKNOWN6_SKIPPABLE: u32 = 0x0309_2023;
    const VALIDATION3_SKIPPABLE: u32 = 0x0309_2025;
    const UNKNOWN7_SKIPPABLE: u32 = 0x0309_2026;
    const TITLE_ID_SKIPPABLE: u32 = 0x0309_2028;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
struct CGameCtnBlockSkin<'node> {
    cursor: BodyCursor<'node>,
    pub foreground_pack_desc: Option<&'node str>,
    pub pack_desc: Option<&'node str>,
    pub parent_pack_desc: Option<&'node str>,
    pub text: Option<&'node str>,
}

impl<'node> FromNode<'node> for CGameCtnBlockSkin<'node> {
    const CLASS_ID: u32 = 0x0305_9000;
}

impl CGameCtnBlockSkin<'_> {
    const TEXT: u32 = 0x0305_9000;
    const SKIN: u32 = 0x0305_9001;
    const SKIN_AND_PARENT: u32 = 0x0305_9002;
    const SKIN2: u32 = 0x0305_9003;
}

impl<'node> sealed::FromNodeInner<'node> for CGameCtnBlockSkin<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError>
    where
        Self: Sized,
    {
        Ok(CGameCtnBlockSkin {
            cursor,
            foreground_pack_desc: None,
            pack_desc: None,
            parent_pack_desc: None,
            text: None,
        })
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            Self::TEXT => {
                self.text = Some(
                    self.cursor
                        .read_string()
                        .context("Reading block text data")?,
                );
                let _unknown = self
                    .cursor
                    .read_string()
                    .context("Reading block text unknown")?;
            }

            Self::SKIN => {
                self.text = Some(
                    self.cursor
                        .read_string()
                        .context("Reading block skin text data")?,
                );
                self.pack_desc = Some(
                    self.cursor
                        .read_file_reference()
                        .context("Reading block skin pack desc")?,
                );
            }

            Self::SKIN_AND_PARENT => {
                self.text = Some(
                    self.cursor
                        .read_string()
                        .context("Reading block skin/parent text data")?,
                );
                self.pack_desc = Some(
                    self.cursor
                        .read_file_reference()
                        .context("Reading block skin/parent pack desc")?,
                );
                self.parent_pack_desc = Some(
                    self.cursor
                        .read_file_reference()
                        .context("Reading block skin/parent parent pack desc")?,
                );
            }

            Self::SKIN2 => {
                let _version = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading secondary skin version")?;
                self.foreground_pack_desc = Some(
                    self.cursor
                        .read_file_reference()
                        .context("Reading secondary skin foreground pack desc")?,
                );
            }

            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }

        Ok(())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
struct CGameWaypointSpecialProperty<'node> {
    cursor: BodyCursor<'node>,
    pub order: Option<u32>,
    pub spawn: Option<u32>,
    pub tag: Option<&'node str>,
}

impl<'node> FromNode<'node> for CGameWaypointSpecialProperty<'node> {
    const CLASS_ID: u32 = 0x2e00_9000;
}

impl CGameWaypointSpecialProperty<'_> {
    const WAYPOINT_DATA: u32 = 0x2e00_9000;
}

impl<'node> sealed::FromNodeInner<'node> for CGameWaypointSpecialProperty<'node> {
    fn from_node(cursor: BodyCursor<'node>) -> Result<Self, GbxError>
    where
        Self: Sized,
    {
        Ok(CGameWaypointSpecialProperty {
            cursor,
            order: None,
            spawn: None,
            tag: None,
        })
    }

    fn cursor(&self) -> &BodyCursor<'node> {
        &self.cursor
    }

    fn cursor_mut(&mut self) -> &mut BodyCursor<'node> {
        &mut self.cursor
    }

    fn handle_chunk(&mut self, wrapped_chunk_id: u32) -> Result<(), GbxError> {
        match wrapped_chunk_id {
            Self::WAYPOINT_DATA => {
                let version = self
                    .cursor
                    .read_u32::<LE>()
                    .context("Reading waypoint data version")?;
                if version == 1 {
                    self.spawn = Some(
                        self.cursor
                            .read_u32::<LE>()
                            .context("Reading waypoint spawn")?,
                    );
                    self.order = Some(
                        self.cursor
                            .read_u32::<LE>()
                            .context("Reading waypoint order")?,
                    );
                } else {
                    self.tag = Some(self.cursor.read_string().context("Reading waypoint tag")?);
                    self.order = Some(
                        self.cursor
                            .read_u32::<LE>()
                            .context("Reading waypoint order")?,
                    );
                }
            }

            _ => {
                return Err(GbxErrorInner::InvalidChunkForClass {
                    chunk_id: wrapped_chunk_id,
                    class_id: Self::CLASS_ID,
                }
                .into())
            }
        }

        Ok(())
    }
}
