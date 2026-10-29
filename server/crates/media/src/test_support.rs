//! Fixtures and a reader that counts what it hands out, to check the walkers
//! skip rather than read.

use std::{
    cell::Cell,
    io::{self, BufReader, Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};

use crate::mime::SNIFF_LEN;

/// A [`Cursor`] that adds every byte it returns to a shared counter.
pub struct CountingReader {
    inner: Cursor<Vec<u8>>,
    read: Rc<Cell<usize>>,
}

impl Read for CountingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.set(self.read.get() + n);
        Ok(n)
    }
}

impl Seek for CountingReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// `data` behind a page-sized buffer, like a probed file, and the count of
/// bytes read from it.
pub fn page_reader(data: Vec<u8>) -> (BufReader<CountingReader>, Rc<Cell<usize>>) {
    let read = Rc::new(Cell::new(0));
    let reader = CountingReader {
        inner: Cursor::new(data),
        read: Rc::clone(&read),
    };
    (BufReader::with_capacity(SNIFF_LEN, reader), read)
}

/// A big-endian TIFF structure, as EXIF blocks hold, whose first directory
/// has `orientation` after another tag.
pub fn tiff(orientation: u16) -> Vec<u8> {
    let mut data = b"MM\0*\0\0\0\x08\0\x02".to_vec();
    data.extend([0x01, 0x0F, 0, 2, 0, 0, 0, 4, b'A', b'B', b'C', 0]);
    data.extend([0x01, 0x12, 0, 3, 0, 0, 0, 1]);
    data.extend(orientation.to_be_bytes());
    data.extend([0, 0, 0, 0, 0, 0]);
    data
}
