use std::{
    io::{self, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

use crate::Key;

use super::{
    chunk,
    error::ContentError,
    format::{self, CHUNK_SIZE, HEADER_SIZE, Layout},
    keys::{self, Keys},
    read::io_error,
};

/// A decrypted view of an encrypted file for asynchronous readers.
///
/// Opening it does not authenticate the whole file. A seek jumps to the chunk
/// that contains the plaintext offset.
pub struct AsyncContentFile<R> {
    inner: R,
    keys: Keys,
    header: [u8; HEADER_SIZE as usize],
    layout: Layout,
    pos: u64,
    cached: Option<Cached>,
    load: Option<Load>,
}

struct Cached {
    index: u64,
    plain: Vec<u8>,
}

enum Load {
    Seek {
        index: u64,
        started: bool,
    },
    Read {
        index: u64,
        buf: Vec<u8>,
        filled: usize,
    },
}

enum Advance {
    Continue,
    Pending,
    Error(ContentError),
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncContentFile<R> {
    pub async fn open(key: &Key, mut inner: R) -> Result<Self, ContentError> {
        let mut header = [0; HEADER_SIZE as usize];
        read_exact(&mut inner, &mut header).await?;
        format::check_header(&header)?;
        let layout = format::layout(format::plain_len(&header))?;
        Ok(Self {
            inner,
            keys: keys::derive(&key.contents),
            header,
            layout,
            pos: 0,
            cached: None,
            load: None,
        })
    }

    pub fn plaintext_len(&self) -> u64 {
        self.layout.plain_len
    }

    fn poll_fill(
        &mut self,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), ContentError>> {
        loop {
            if out.remaining() == 0 || self.pos >= self.layout.plain_len {
                return Poll::Ready(Ok(()));
            }
            if self.copy_cached(out) {
                return Poll::Ready(Ok(()));
            }
            match self.advance(cx) {
                Advance::Continue => {}
                Advance::Pending => return Poll::Pending,
                Advance::Error(error) => return Poll::Ready(Err(error)),
            }
        }
    }

    fn copy_cached(&mut self, out: &mut ReadBuf<'_>) -> bool {
        let index = self.pos / CHUNK_SIZE;
        let within = (self.pos % CHUNK_SIZE) as usize;
        let Some(cached) = &self.cached else {
            return false;
        };
        if cached.index != index {
            return false;
        }
        let n = (cached.plain.len() - within).min(out.remaining());
        out.put_slice(&cached.plain[within..within + n]);
        self.pos += n as u64;
        true
    }

    fn advance(&mut self, cx: &mut Context<'_>) -> Advance {
        match self.load.take() {
            None => self.start_seek(),
            Some(Load::Seek { index, started }) => self.poll_seek(cx, index, started),
            Some(Load::Read { index, buf, filled }) => self.poll_record(cx, index, buf, filled),
        }
    }

    fn start_seek(&mut self) -> Advance {
        self.load = Some(Load::Seek {
            index: self.pos / CHUNK_SIZE,
            started: false,
        });
        Advance::Continue
    }

    fn poll_seek(&mut self, cx: &mut Context<'_>, index: u64, started: bool) -> Advance {
        let offset = match format::physical_offset(index) {
            Ok(offset) => offset,
            Err(error) => return Advance::Error(error),
        };

        if !started {
            if let Err(error) = Pin::new(&mut self.inner).start_seek(SeekFrom::Start(offset)) {
                return Advance::Error(error.into());
            }
            self.load = Some(Load::Seek {
                index,
                started: true,
            });
            return Advance::Continue;
        }

        match Pin::new(&mut self.inner).poll_complete(cx) {
            Poll::Pending => {
                self.load = Some(Load::Seek {
                    index,
                    started: true,
                });
                Advance::Pending
            }
            Poll::Ready(Err(error)) => Advance::Error(error.into()),
            Poll::Ready(Ok(_)) => {
                let len = format::chunk_plain_len(&self.layout, index) as usize + 16;
                self.load = Some(Load::Read {
                    index,
                    buf: vec![0; len],
                    filled: 0,
                });
                Advance::Continue
            }
        }
    }

    fn poll_record(
        &mut self,
        cx: &mut Context<'_>,
        index: u64,
        mut buf: Vec<u8>,
        filled: usize,
    ) -> Advance {
        let mut rest = ReadBuf::new(&mut buf[filled..]);
        let n = match Pin::new(&mut self.inner).poll_read(cx, &mut rest) {
            Poll::Pending => {
                self.load = Some(Load::Read { index, buf, filled });
                return Advance::Pending;
            }
            Poll::Ready(Err(error)) => return Advance::Error(error.into()),
            Poll::Ready(Ok(())) => rest.filled().len(),
        };

        if n == 0 {
            return Advance::Error(ContentError::Layout);
        }

        let filled = filled + n;
        if filled < buf.len() {
            self.load = Some(Load::Read { index, buf, filled });
            return Advance::Continue;
        }

        match open_record(&self.keys, &self.header, index, &mut buf) {
            Ok(plain) => {
                self.cached = Some(Cached { index, plain });
                self.load = None;
                Advance::Continue
            }
            Err(error) => Advance::Error(error),
        }
    }
}

fn open_record(
    keys: &Keys,
    header: &[u8; HEADER_SIZE as usize],
    index: u64,
    buf: &mut [u8],
) -> Result<Vec<u8>, ContentError> {
    let plain_len = buf.len() - 16;
    let (body, tag) = buf.split_at_mut(plain_len);
    let mut tag_bytes = [0; 16];
    tag_bytes.copy_from_slice(tag);
    chunk::open(keys, header, index, body, &tag_bytes)?;
    Ok(body.to_vec())
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncRead for AsyncContentFile<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.poll_fill(cx, buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => Poll::Ready(result.map_err(io_error)),
        }
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncSeek for AsyncContentFile<R> {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        let len = self.layout.plain_len as i128;
        let next = match position {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::End(offset) => len + offset as i128,
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
        self.load = None;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Poll::Ready(Ok(self.pos))
    }
}

async fn read_exact(
    reader: &mut (impl AsyncRead + Unpin),
    buf: &mut [u8],
) -> Result<(), ContentError> {
    use tokio::io::AsyncReadExt;
    reader.read_exact(buf).await.map(|_| ()).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ContentError::Layout
        } else {
            ContentError::Io(error)
        }
    })
}
