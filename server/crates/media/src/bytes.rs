//! Big-endian reads and skips over a buffered file, for the format walkers.

use std::io::{self, BufRead, BufReader, Read, Seek};

use memchr::memmem::{self, Finder};

pub fn read_array<const N: usize>(reader: &mut impl Read) -> io::Result<[u8; N]> {
    let mut buf = [0; N];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_u8(reader: &mut impl Read) -> io::Result<u8> {
    Ok(read_array::<1>(reader)?[0])
}

pub fn read_u16(reader: &mut impl Read) -> io::Result<u16> {
    read_array(reader).map(u16::from_be_bytes)
}

pub fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    read_array(reader).map(u32::from_be_bytes)
}

pub fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    read_array(reader).map(u64::from_be_bytes)
}

/// Moves `len` bytes forward. Within the buffered page that's free; past it,
/// the next read starts there and the bytes between are never read.
pub fn skip<R: Read + Seek>(reader: &mut BufReader<R>, len: u64) -> io::Result<()> {
    let len = i64::try_from(len).map_err(|_| invalid_data("skip past any file size"))?;
    reader.seek_relative(len)
}

/// Whether any of `needles` occurs in the next `len` bytes. Reads only up to
/// the first hit and skips the rest.
pub fn contains_any<R: Read + Seek>(
    reader: &mut BufReader<R>,
    len: u64,
    needles: &[&[u8]],
) -> io::Result<bool> {
    // The tail of the previous chunk, so a needle across chunks is still found.
    let overlap = needles
        .iter()
        .map(|n| n.len())
        .max()
        .unwrap_or(0)
        .saturating_sub(1);
    let finders: Vec<_> = needles.iter().map(|needle| Finder::new(*needle)).collect();
    let mut window = Vec::new();
    let mut left = len;
    while left > 0 {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let take = chunk.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        window.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        left -= take as u64;

        if finders.iter().any(|finder| finder.find(&window).is_some()) {
            skip(reader, left)?;
            return Ok(true);
        }
        window.drain(..window.len().saturating_sub(overlap));
    }
    Ok(false)
}

pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    memmem::find(haystack, needle).is_some()
}

pub fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn finds_a_needle_split_across_chunks() {
        let mut data = vec![b'.'; 4094];
        data.extend_from_slice(b"needle");
        data.extend_from_slice(b"tail");
        let len = data.len() as u64;
        let mut reader = BufReader::with_capacity(4096, Cursor::new(data));

        assert!(contains_any(&mut reader, len - 4, &[b"needle"]).unwrap());
        assert_eq!(read_array::<4>(&mut reader).unwrap(), *b"tail");
    }

    #[test]
    fn reads_the_whole_range_when_nothing_matches() {
        let mut reader = BufReader::new(Cursor::new(b"abcdefXY".to_vec()));

        assert!(!contains_any(&mut reader, 6, &[b"zz"]).unwrap());
        assert_eq!(read_array::<2>(&mut reader).unwrap(), *b"XY");
    }
}
