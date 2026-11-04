use std::io::{self, Read, Seek, SeekFrom, Write};

use hmac::Mac;

use crate::Key;

use super::{
    chunk,
    error::ContentError,
    format::{self, CHUNK_SIZE, EOF_SIZE, HEADER_SIZE, Layout},
    keys::{self, Keys},
    mac,
};

/// Decrypt `encrypted`, then check the end-of-file authenticator.
///
/// Plaintext from earlier chunks is written before a later chunk or the
/// end-of-file check can fail. Publish `plaintext` only after this returns `Ok`.
pub fn decrypt<R: Read, W: Write>(
    key: &Key,
    encrypted: &mut R,
    encrypted_len: u64,
    plaintext: &mut W,
) -> Result<(), ContentError> {
    let (head, layout) = read_header(encrypted, encrypted_len)?;
    let derived = keys::derive(&key.contents);
    let mut hmac = mac::start(&derived, &head);
    for index in 0..layout.chunk_count {
        let (body, tag) = read_record(encrypted, &layout, index)?;
        let mut body = body;
        chunk::open(&derived, &head, index, &mut body, &tag)?;
        plaintext.write_all(&body)?;
        hmac.update(&tag);
    }

    let mut eof = [0; EOF_SIZE as usize];
    read_exact(encrypted, &mut eof)?;

    mac::matches(
        &eof,
        &mac::finish(hmac, layout.plain_len, layout.chunk_count),
    )
}

/// A decrypted view of an encrypted file. Opening it does not authenticate the whole file.
pub struct ContentFile<R> {
    inner: R,
    keys: Keys,
    header: [u8; HEADER_SIZE as usize],
    layout: Layout,
    pos: u64,
    cached: Option<Cached>,
}

struct Cached {
    index: u64,
    plain: Vec<u8>,
}

// Own ContentFile methods.
impl<R: Read + Seek> ContentFile<R> {
    pub fn open(key: &Key, mut inner: R) -> Result<Self, ContentError> {
        // A trick to get the file length. It doesn't read the disk.
        let len = inner.seek(SeekFrom::End(0))?;
        inner.seek(SeekFrom::Start(0))?;
        let (header, layout) = read_header(&mut inner, len)?;
        Ok(Self {
            inner,
            keys: keys::derive(&key.contents),
            header,
            layout,
            pos: 0,
            cached: None,
        })
    }

    pub fn plaintext_len(&self) -> u64 {
        self.layout.plain_len
    }

    /// Determines which chunk serves the `index` position; then reads, decrypt
    /// and cache the chunk.
    fn load(&mut self, index: u64) -> Result<(), ContentError> {
        if self
            .cached
            .as_ref()
            .is_some_and(|cached| cached.index == index)
        {
            return Ok(());
        }

        let offset = format::physical_offset(index)?;
        self.inner.seek(SeekFrom::Start(offset))?;
        let (mut body, tag) = read_record(&mut self.inner, &self.layout, index)?;
        chunk::open(&self.keys, &self.header, index, &mut body, &tag)?;
        self.cached = Some(Cached { index, plain: body });
        Ok(())
    }
}

// Implement Read and Seek for ContentFile.
impl<R: Read + Seek> Read for ContentFile<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.layout.plain_len {
            return Ok(0);
        }

        let index = self.pos / CHUNK_SIZE;
        self.load(index).map_err(io_error)?;

        let cached = self.cached.as_ref().expect("chunk was loaded");
        let within = (self.pos % CHUNK_SIZE) as usize;
        let n = (cached.plain.len() - within).min(buf.len());
        buf[..n].copy_from_slice(&cached.plain[within..within + n]);
        self.pos += n as u64;
        // If `n` is not enough super will call read again.
        Ok(n)
    }
}

// Implement Seek for ContentFile.
impl<R: Read + Seek> Seek for ContentFile<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::End(offset) => self.layout.plain_len as i128 + offset as i128,
            SeekFrom::Current(offset) => self.pos as i128 + offset as i128,
        };
        if next < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start",
            ));
        }
        self.pos = u64::try_from(next)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek past any file size"))?;
        Ok(self.pos)
    }
}

fn read_header<R: Read>(
    reader: &mut R,
    file_len: u64,
) -> Result<([u8; HEADER_SIZE as usize], Layout), ContentError> {
    let layout = format::layout(file_len)?;
    let mut header = [0; HEADER_SIZE as usize];
    read_exact(reader, &mut header)?;
    format::check_header(&header)?;
    Ok((header, layout))
}

fn read_record<R: Read>(
    reader: &mut R,
    layout: &Layout,
    index: u64,
) -> Result<(Vec<u8>, [u8; 16]), ContentError> {
    let plain_len = format::chunk_plain_len(layout, index) as usize;
    let mut body = vec![0; plain_len];
    read_exact(reader, &mut body)?;
    let mut tag = [0; 16];
    read_exact(reader, &mut tag)?;
    Ok((body, tag))
}

fn read_exact(reader: &mut impl Read, buf: &mut [u8]) -> Result<(), ContentError> {
    reader.read_exact(buf).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ContentError::Layout
        } else {
            ContentError::Io(error)
        }
    })
}

pub(super) fn io_error(error: ContentError) -> io::Error {
    match error {
        ContentError::Io(error) => error,
        other => io::Error::new(io::ErrorKind::InvalidData, other),
    }
}
