use crate::{
    parse::{self, CGame},
    Context, GbxError, GbxErrorInner, Meta,
};
use byteorder::{ReadBytesExt, LE};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{Cursor, Seek},
    ops::{Deref, DerefMut},
};

pub trait CursorExt {
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
pub(crate) struct BodyCursor<'node> {
    inner: Cursor<&'node [u8]>,
    lookback_version: Option<u32>,
    strings: Vec<&'node str>,
    nodes: HashMap<i32, CGame<'node>>,
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
    pub fn new(cursor: Cursor<&'node [u8]>) -> Self {
        BodyCursor {
            inner: cursor,
            lookback_version: None,
            strings: Vec::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn read_string(&mut self) -> Result<&'node str, GbxError> {
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

    pub fn read_meta(&mut self) -> Result<Meta<'node>, GbxError> {
        Ok(Meta {
            id: self.read_lookback_string().context("Reading meta ID")?,
            collection: self
                .read_lookback_string()
                .context("Reading meta collection")?,
            author: self.read_lookback_string().context("Reading meta author")?,
        })
    }

    pub fn read_lookback_string(&mut self) -> Result<&'node str, GbxError> {
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

    pub fn expect_node_ref<N: parse::Parsable<'node>>(&mut self) -> Result<Option<N>, GbxError> {
        let Some(node) = self.read_node_ref()? else {
            return Ok(None);
        };
        Ok(Some(N::coerce(node)?))
    }

    pub fn read_node_ref(&mut self) -> Result<Option<CGame<'node>>, GbxError> {
        let index = self
            .read_i32::<LE>()
            .context("Reading node reference index")?;

        if index == -1 {
            return Ok(None);
        }

        if let Some(cgame) = self.nodes.get(&index) {
            return Ok(Some(cgame.clone()));
        }

        if index >= 0 {
            let class_id = self
                .read_u32::<LE>()
                .context("Reading node reference class ID")?;

            let position = self.position() as usize;

            let mut cursor = BodyCursor::new(Cursor::new(&self.get_ref()[position..]));
            let node = CGame::parse(&mut cursor, class_id)?;

            let forward = cursor.position() as i64;
            tracing::trace!("read {} bytes of {:08x}", forward, class_id);
            self.seek_relative(forward)
                .context("Seeking after read node reference")?;

            self.nodes.insert(index, node.clone());

            Ok(Some(node))
        } else {
            unreachable!("index < 0: {index}");
        }
    }

    pub fn read_int3(&mut self) -> Result<[u32; 3], GbxError> {
        Ok([
            self.read_u32::<LE>()?,
            self.read_u32::<LE>()?,
            self.read_u32::<LE>()?,
        ])
    }

    pub fn read_byte3(&mut self) -> Result<[u8; 3], GbxError> {
        Ok([self.read_u8()?, self.read_u8()?, self.read_u8()?])
    }

    pub fn read_file_ref(&mut self) -> Result<&'node str, GbxError> {
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

    pub fn force_skip(&mut self, skip_chunk_id: u32) -> Result<(), GbxError> {
        loop {
            let next_chunk_id_maybe = self.peek_u32_le().context("Force skipping chunk")?;
            if (next_chunk_id_maybe & 0xffff_ff00) == (skip_chunk_id & 0xffff_ff00) {
                tracing::warn!("Force skipped chunk {:08x}", skip_chunk_id);
                break;
            }
            if next_chunk_id_maybe == 0xfacade01
                && self.peek_u32_le().is_err_and(|err| match &*err {
                    GbxErrorInner::Io(io) => {
                        matches!(io.kind(), std::io::ErrorKind::UnexpectedEof)
                    }
                    _ => false,
                })
            {
                tracing::warn!("Force skipped chunk {:08x}", skip_chunk_id);
                tracing::warn!("Reached end of file early");
                break;
            }
            let _skipped_byte = self.read_u8().context("Force skipping media tracker")?;
        }

        Ok(())
    }
}
