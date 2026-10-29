//! WebP: orientation from the EXIF chunk. Only extended files, which open with
//! a `VP8X` chunk, have one, and it usually follows the image data. Chunk
//! bodies are skipped.

use std::io::{self, BufReader, Read, Seek};

use crate::{
    bytes::{invalid_data, read_array, read_u8, skip},
    exif,
};

/// The `VP8X` flag for an EXIF chunk.
const HAS_EXIF: u8 = 0x08;
const UPRIGHT: u32 = 1;

/// Expects the reader at the start of the file.
pub fn read_orientation<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<u32> {
    let header: [u8; 12] = read_array(reader)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WEBP" {
        return Err(invalid_data("not a WebP"));
    }
    let (kind, len) = chunk_header(reader)?;
    if kind != *b"VP8X" || len == 0 {
        return Ok(UPRIGHT);
    }
    if read_u8(reader)? & HAS_EXIF == 0 {
        return Ok(UPRIGHT);
    }
    skip(reader, padded(len) - 1)?;
    loop {
        let (kind, len) = match chunk_header(reader) {
            Ok(header) => header,
            // The flag promised a chunk the file doesn't have.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(UPRIGHT),
            Err(e) => return Err(e),
        };
        if kind == *b"EXIF" {
            return exif::read_orientation(reader, len);
        }
        skip(reader, padded(len))?;
    }
}

fn chunk_header(reader: &mut impl Read) -> io::Result<([u8; 4], u64)> {
    let kind = read_array(reader)?;
    let len = u32::from_le_bytes(read_array(reader)?);
    Ok((kind, len.into()))
}

/// Chunk bodies are padded to an even length.
fn padded(len: u64) -> u64 {
    len + len % 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{page_reader, tiff};

    fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let len = u32::try_from(body.len()).unwrap();
        let padding = if body.len() % 2 == 1 { &[0][..] } else { &[] };
        [kind, &len.to_le_bytes()[..], body, padding].concat()
    }

    fn webp(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body = [b"WEBP".to_vec(), chunks.concat()].concat();
        let len = u32::try_from(body.len()).unwrap();
        [b"RIFF".to_vec(), len.to_le_bytes().to_vec(), body].concat()
    }

    fn vp8x(flags: u8) -> Vec<u8> {
        chunk(b"VP8X", &[flags, 0, 0, 0, 99, 0, 0, 99, 0, 0])
    }

    #[test]
    fn finds_the_orientation_past_the_image_data() {
        let file = webp(&[
            vp8x(HAS_EXIF),
            chunk(b"VP8 ", &vec![0xAB; 100_001]),
            chunk(b"EXIF", &tiff(6)),
        ]);
        let (mut reader, read) = page_reader(file);
        assert_eq!(read_orientation(&mut reader).unwrap(), 6);
        assert!(read.get() < 10_000, "read {} bytes", read.get());
    }

    #[test]
    fn is_upright_without_exif() {
        let flagless = webp(&[vp8x(0), chunk(b"EXIF", &tiff(6))]);
        let (mut reader, _) = page_reader(flagless);
        assert_eq!(read_orientation(&mut reader).unwrap(), UPRIGHT);

        let simple = webp(&[chunk(b"VP8L", &[0; 30])]);
        let (mut reader, _) = page_reader(simple);
        assert_eq!(read_orientation(&mut reader).unwrap(), UPRIGHT);

        let missing = webp(&[vp8x(HAS_EXIF), chunk(b"VP8 ", &[0; 30])]);
        let (mut reader, _) = page_reader(missing);
        assert_eq!(read_orientation(&mut reader).unwrap(), UPRIGHT);
    }
}
