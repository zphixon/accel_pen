use byteorder::{ReadBytesExt, LE};
use cursor::BodyCursor;
use std::{
    fmt::{Debug, Display},
    io::{Cursor, Read, Seek},
    ops::Deref,
};

mod cursor;
pub mod parse;

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

    #[error("Could not parse class with ID {0:08x}")]
    InvalidClass(u32),

    #[error("Wanted to parse class ID {wanted:08x}, had class ID {had:08x} instead")]
    IncorrectType { wanted: u32, had: u32 },

    #[error("Invalid UTF-8")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("Invalid lookback string, file may be corrupted")]
    InvalidLookbackString,

    #[error("Invalid chunk {chunk_id:08x} for class {class_id:08x}")]
    InvalidChunkForClass { chunk_id: u32, class_id: u32 },

    #[error("Invalid string from {start:08x} to {end:08x}")]
    InvalidString { start: usize, end: usize },

    #[error("Invalid node reference")]
    InvalidNodeRef,
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

#[allow(unused)]
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

#[allow(unused)]
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

pub struct Node<'data> {
    header: Header,
    data: &'data [u8],
    body: Vec<u8>,
}

impl Debug for Node<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("header", &self.header)
            .field("body", &"...")
            .finish()
    }
}

impl<'data> Node<'data> {
    pub fn read_from<B: AsRef<[u8]> + std::panic::UnwindSafe + 'data>(
        data: &'data B,
    ) -> Result<Node<'data>, GbxError> {
        let data = data.as_ref();
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

        let body = lzokay_native::decompress(&mut cursor, None).context("Decompressing body")?;
        //let Ok(body) = std::panic::catch_unwind(move || {
        //    let mut cursor = cursor;

        //    #[allow(unexpected_cfgs)]
        //    if cfg!(fuzzing) {
        //        let position = cursor.position() as usize;
        //        let inner: B = cursor.into_inner();
        //        Ok(Vec::from(&inner.as_ref()[position..]))
        //    } else {
        //        lzokay_native::decompress(&mut cursor, None).context("Decompressing body")
        //    }
        //}) else {
        //    return Err(GbxErrorInner::Lzo(lzokay_native::Error::Unknown).into());
        //};

        //let body = body?;
        Ok(Node { header, data, body })
    }

    pub fn parse(&self) -> Result<parse::CGame, GbxError> {
        let mut this = parse::CGame::parse(
            &mut BodyCursor::new(Cursor::new(&self.body)),
            self.header.class_id,
        )?;

        let mut cursor = BodyCursor::new(Cursor::new(self.data));
        for header_chunk in self.header.chunks.iter() {
            tracing::trace!("Parsing header chunk {:08x}", header_chunk.id);
            cursor.set_position(header_chunk.data_start);
            this.parse_one(&mut cursor, self.header.class_id | header_chunk.id)?;
        }

        Ok(this)
    }
}

#[derive(Debug, Clone)]
pub struct Meta<'node> {
    pub id: &'node str,
    pub collection: &'node str,
    pub author: &'node str,
}
