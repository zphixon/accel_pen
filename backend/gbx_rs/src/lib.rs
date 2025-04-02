use byteorder::{ReadBytesExt, LE};
use std::{
    fmt::Debug,
    io::{Cursor, Read, Seek},
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
    cursor.read_exact(&mut magic).context("magic")?;
    if magic != *b"GBX" {
        tracing::debug!("magic {:?}", magic);
        return Err(GbxErrorInner::NotGbx.into());
    }

    let version = cursor.read_u16::<LE>().context("version")?;
    if version < 3 {
        return Err(GbxErrorInner::VersionNotSupported(version).into());
    }
    tracing::debug!("version {}", version);

    let byte_format = cursor.read_u8().context("byte format")?;
    tracing::debug!("byte format {:?}", byte_format as char);
    let byte_format = byte_format.try_into().context("byte format")?;

    let _ref_table_compression = cursor.read_u8().context("ref table compression")?;
    tracing::debug!("ref table compression {:?}", _ref_table_compression as char);
    let _ref_table_compression: Compression = _ref_table_compression
        .try_into()
        .context("ref table compression")?;

    let body_compression = cursor.read_u8().context("body compression")?;
    tracing::debug!("body compression {:?}", body_compression as char);
    let body_compression = body_compression.try_into().context("body compression")?;

    if version >= 4 {
        let _unknown = cursor.read_u8().context("unknown 1")?;
        tracing::debug!("unknown {}", _unknown);
    }

    let class_id = cursor.read_u32::<LE>().context("class id")?;
    tracing::debug!("class id 0x{:08x}", class_id);

    if version >= 6 {
        let _user_data_size = cursor.read_u32::<LE>().context("user data size")?;
        tracing::debug!("user data size {}", _user_data_size);
    }

    let num_header_chunks = cursor.read_u32::<LE>().context("num header chunks")?;
    if num_header_chunks == 0 {
        return Err(GbxErrorInner::NoHeaderChunks.into());
    }

    tracing::debug!("num header chunks {}", num_header_chunks);

    let mut chunks = Vec::new();
    for i in 0..num_header_chunks {
        tracing::debug!("chunk {}", i);

        let id = cursor
            .read_u32::<LE>()
            .with_context(|| format!("chunk {i} id"))?
            & 0x0fff;
        tracing::debug!("  chunk id 0x{:08x}", id);

        let chunk_size_heavy = cursor
            .read_u32::<LE>()
            .with_context(|| format!("chunk {i} size"))?;
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
            .with_context(|| format!("chunk {} data", chunk.id))?;
    }

    let num_nodes = cursor.read_u32::<LE>().context("num nodes")?;
    tracing::debug!("num nodes {}", num_nodes);
    let num_external_nodes = cursor.read_u32::<LE>().context("num external nodes")?;
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

pub struct Node<B> {
    header: Header,
    cursor: Cursor<B>,
    body_start: u64,
    body: Option<Vec<u8>>,
}

impl<B> Debug for Node<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("header", &self.header)
            .field("body", &"...")
            .finish()
    }
}

impl<B: AsRef<[u8]>> Node<B> {
    pub fn read_from(mut cursor: Cursor<B>) -> Result<Node<B>, GbxError> {
        let header = parse_header(&mut cursor).context("parse header")?;
        if header.body_compression != Compression::Compressed {
            return Err(GbxErrorInner::NotCompressed.into());
        }

        let uncompressed_size = cursor.read_u32::<LE>().context("uncompressed size")?;
        tracing::debug!("uncompressed size {}", uncompressed_size);
        let compressed_size = cursor.read_u32::<LE>().context("compressed size")?;
        tracing::debug!("compressed size {}", compressed_size);
        let body_start = cursor.position();
        tracing::debug!("body start {}", body_start);

        Ok(Node {
            header,
            cursor,
            body_start,
            body: None,
        })
    }

    pub fn decompress(&mut self) -> Result<(), GbxError> {
        self.body =
            Some(lzokay_native::decompress(&mut self.cursor, None).context("decompress body")?);
        Ok(())
    }

    pub fn to<'this, N: FromNode<'this, B>>(&'this mut self) -> Option<N> {
        if self.header.class_id != N::CLASS_ID {
            None
        } else {
            N::from_node(self)
        }
    }
}

pub trait FromNode<'node, B: AsRef<[u8]>> {
    const CLASS_ID: u32;

    fn from_node(node: &'node mut Node<B>) -> Option<Self>
    where
        Self: Sized;
}

pub struct CGameCtnChallenge<'node, B: AsRef<[u8]>> {
    node: &'node mut Node<B>,
    map_name: Option<&'node str>,
}

impl<'node, B: AsRef<[u8]>> FromNode<'node, B> for CGameCtnChallenge<'node, B> {
    const CLASS_ID: u32 = 0x0304_3000;

    fn from_node(node: &'node mut Node<B>) -> Option<Self> {
        if node.header.chunks.is_empty() {
            return None;
        }
        Some(Self {
            node,
            map_name: None,
        })
    }
}

impl<'node, B: AsRef<[u8]>> CGameCtnChallenge<'node, B> {
    const MAP_INFORMATION1: u32 = 0x0000_0002;
    const MAP_INFORMATION2: u32 = 0x0000_0003;

    fn position(&mut self, chunk_id: u32) -> Result<(), GbxError> {
        for (i, chunk) in self.node.header.chunks.iter().enumerate() {
            self.node.cursor.set_position(chunk.data_start);
            let current_id = self
                .node
                .cursor
                .read_u32::<LE>()
                .with_context(|| format!("looking for {chunk_id} in chunk {i}"))?;
            if current_id == chunk_id & Self::CLASS_ID {
                return Ok(());
            }
        }

        Err(GbxErrorInner::NoSuchChunk(chunk_id).into())
    }

    fn read_chunk(&mut self, chunk_id: u32) -> Result<(), GbxError> {
        Ok(())
    }

    pub fn map_name(&mut self) -> Result<&str, GbxError> {
        if let Some(map_name) = self.map_name {
            return Ok(map_name);
        }

        if self.node.header.version <= 2 {
            self.read_chunk(Self::MAP_INFORMATION1);
        } else {
            self.read_chunk(Self::MAP_INFORMATION2);
        }

        Ok(self.map_name.unwrap())
    }
}
