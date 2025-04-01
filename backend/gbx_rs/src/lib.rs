use std::fmt::Debug;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

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
}

#[derive(Debug)]
pub struct Header {
    pub version: u16,
    pub byte_format: u8,
    pub body_compression: u8,
    pub class_id: u32,
    pub header_chunks: Vec<HeaderChunk>,
    pub num_nodes: u32,
    pub num_external_nodes: u32,
}

pub struct HeaderChunk {
    pub chunk_id: u32,
    pub chunk_size: u32,
    pub is_heavy: bool,
    pub data: Vec<u8>,
}

impl Debug for HeaderChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderChunk")
            .field("chunk_id", &self.chunk_id)
            .field("chunk_size", &self.chunk_size)
            .field("is_heavy", &self.is_heavy)
            .field("data", &"...")
            .finish()
    }
}

pub async fn parse_headers(
    mut stream: BufReader<impl AsyncRead + Unpin>,
) -> Result<Header, GbxError> {
    let mut magic = [0u8; 3];
    stream.read_exact(&mut magic).await.context("magic")?;
    if magic != *b"GBX" {
        tracing::debug!("magic {:?}", magic);
        return Err(GbxErrorInner::NotGbx.into());
    }

    let version = stream.read_u16_le().await.context("version")?;
    if version < 3 {
        return Err(GbxErrorInner::VersionNotSupported(version).into());
    }
    tracing::debug!("version {}", version);

    let byte_format = stream.read_u8().await.context("byte format")?;
    tracing::debug!("byte format {:?}", byte_format as char);

    let _ref_table_compression = stream.read_u8().await.context("ref table compression")?;
    tracing::debug!("ref table compression {:?}", _ref_table_compression as char);

    let body_compression = stream.read_u8().await.context("body compression")?;
    tracing::debug!("body compression {:?}", body_compression as char);

    if version >= 4 {
        let _unknown = stream.read_u8().await.context("unknown 1")?;
        tracing::debug!("unknown {}", _unknown);
    }

    let class_id = stream.read_u32_le().await.context("class id")?;
    tracing::debug!("class id 0x{:08x}", class_id);

    if version >= 6 {
        let _user_data_size = stream.read_u32_le().await.context("user data size")?;
        tracing::debug!("user data size {}", _user_data_size);
    }

    let num_header_chunks = stream.read_u32_le().await.context("num header chunks")?;
    if num_header_chunks == 0 {
        return Err(GbxErrorInner::NoHeaderChunks.into());
    }

    tracing::debug!("num header chunks {}", num_header_chunks);

    let mut header_chunks = Vec::new();
    for i in 0..num_header_chunks {
        tracing::debug!("chunk {}", i);

        let chunk_id = stream
            .read_u32_le()
            .await
            .with_context(|| format!("chunk {i} id"))?
            & 0xfff;
        tracing::debug!("  chunk id 0x{:08x}", chunk_id);

        let chunk_size = stream
            .read_u32_le()
            .await
            .with_context(|| format!("chunk {i} size"))?
            & !0x8000_0000;
        tracing::debug!("  chunk size {}", chunk_size);

        let is_heavy = (chunk_size & 0x8000_0000) != 0;
        tracing::debug!("  heavy {}", is_heavy);

        header_chunks.push(HeaderChunk {
            chunk_id,
            chunk_size,
            is_heavy,
            data: vec![0; chunk_size as usize],
        });
    }

    for chunk in header_chunks.iter_mut() {
        stream
            .read_exact(&mut chunk.data)
            .await
            .with_context(|| format!("chunk {} data", chunk.chunk_id))?;
    }

    let num_nodes = stream.read_u32_le().await.context("num nodes")?;
    tracing::debug!("num nodes {}", num_nodes);
    let num_external_nodes = stream.read_u32_le().await.context("num external nodes")?;
    tracing::debug!("num external nodes {}", num_external_nodes);

    Ok(Header {
        version,
        byte_format,
        body_compression,
        class_id,
        header_chunks,
        num_nodes,
        num_external_nodes,
    })
}
