//! WebP: animation from the `VP8X` chunk's flags, orientation from the EXIF
//! chunk. Only extended files, which open with `VP8X`, have either, and EXIF
//! usually follows the image data. Chunk bodies are skipped.

use std::io::{self, BufReader, Read, Seek};

use crate::{
    bytes::{invalid_data, read_array, read_u8, skip},
    exif,
};

/// `VP8X` flags.
const HAS_ANIMATION: u8 = 0x02;
const HAS_EXIF: u8 = 0x08;
const UPRIGHT: u32 = 1;

/// What a WebP's headers tell beyond its size.
#[derive(Debug, PartialEq, Eq)]
pub struct Extended {
    pub orientation: u32,
    pub animated: bool,
}

/// Expects the reader at the start of the file.
pub fn read_extended<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<Extended> {
    let header: [u8; 12] = read_array(reader)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WEBP" {
        return Err(invalid_data("not a WebP"));
    }
    let mut found = Extended {
        orientation: UPRIGHT,
        animated: false,
    };
    let (kind, len) = chunk_header(reader)?;
    if kind != *b"VP8X" || len == 0 {
        return Ok(found);
    }
    let flags = read_u8(reader)?;
    found.animated = flags & HAS_ANIMATION != 0;
    if flags & HAS_EXIF == 0 {
        return Ok(found);
    }
    skip(reader, padded(len) - 1)?;
    loop {
        let (kind, len) = match chunk_header(reader) {
            Ok(header) => header,
            // The flag promised a chunk the file doesn't have.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(found),
            Err(e) => return Err(e),
        };
        if kind == *b"EXIF" {
            found.orientation = exif::read_orientation(reader, len)?;
            return Ok(found);
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

    fn read(file: Vec<u8>) -> Extended {
        let (mut reader, _) = page_reader(file);
        read_extended(&mut reader).unwrap()
    }

    const STILL_UPRIGHT: Extended = Extended {
        orientation: UPRIGHT,
        animated: false,
    };

    #[test]
    fn finds_the_orientation_past_the_image_data() {
        let file = webp(&[
            vp8x(HAS_EXIF | HAS_ANIMATION),
            chunk(b"ANMF", &vec![0xAB; 100_001]),
            chunk(b"EXIF", &tiff(6)),
        ]);
        let (mut reader, read) = page_reader(file);
        let found = read_extended(&mut reader).unwrap();
        assert_eq!(
            found,
            Extended {
                orientation: 6,
                animated: true
            }
        );
        assert!(read.get() < 10_000, "read {} bytes", read.get());
    }

    #[test]
    fn reads_the_animation_flag() {
        let found = read(webp(&[vp8x(HAS_ANIMATION), chunk(b"ANMF", &[0; 30])]));
        assert!(found.animated);
    }

    #[test]
    fn is_upright_and_still_without_flags() {
        assert_eq!(
            read(webp(&[vp8x(0), chunk(b"EXIF", &tiff(6))])),
            STILL_UPRIGHT
        );
        assert_eq!(read(webp(&[chunk(b"VP8L", &[0; 30])])), STILL_UPRIGHT);
        assert_eq!(
            read(webp(&[vp8x(HAS_EXIF), chunk(b"VP8 ", &[0; 30])])),
            STILL_UPRIGHT
        );
    }
}
