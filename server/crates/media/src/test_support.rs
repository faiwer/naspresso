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
