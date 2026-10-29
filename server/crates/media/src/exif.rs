//! Orientation as EXIF and TIFF store it: 1 is upright, 5 to 8 turn the image
//! a quarter. EXIF is a TIFF structure; its first directory has the tag.

use std::io::{self, BufReader, Read, Seek};

use crate::bytes::{read_array, skip};

const ORIENTATION_TAG: u16 = 0x0112;
const UPRIGHT: u32 = 1;
const ENTRY_LEN: u64 = 12;

/// The size of a `width`×`height` image once turned upright by `orientation`.
pub fn upright((width, height): (u32, u32), orientation: u32) -> (u32, u32) {
    if matches!(orientation, 5..=8) {
        (height, width)
    } else {
        (width, height)
    }
}

/// The orientation of the TIFF structure filling the next `len` bytes; upright
/// if it has none or isn't one. Reads its header and first directory, and
/// leaves the reader past the `len` bytes.
pub fn read_orientation<R: Read + Seek>(reader: &mut BufReader<R>, len: u64) -> io::Result<u32> {
    let mut at = 0;
    let orientation = find_orientation(reader, len, &mut at)?;
    skip(reader, len - at)?;
    Ok(orientation
        .map(u32::from)
        .filter(|orientation| (1..=8).contains(orientation))
        .unwrap_or(UPRIGHT))
}

/// The Orientation tag's value, reading no further than `len`, with `at`
/// following how far it got.
fn find_orientation<R: Read + Seek>(
    reader: &mut BufReader<R>,
    len: u64,
    at: &mut u64,
) -> io::Result<Option<u16>> {
    if len < 8 {
        return Ok(None);
    }
    let header: [u8; 8] = read_array(reader)?;
    *at = 8;
    let little_endian = match &header[..4] {
        b"II*\0" => true,
        b"MM\0*" => false,
        _ => return Ok(None),
    };
    let u16_at = |bytes: &[u8]| {
        let bytes = [bytes[0], bytes[1]];
        if little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        }
    };
    let ifd = {
        let bytes = [header[4], header[5], header[6], header[7]];
        u64::from(if little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };
    // Writers put the first directory right after the header; one before it is broken.
    if ifd < *at || ifd + 2 > len {
        return Ok(None);
    }
    skip(reader, ifd - *at)?;
    let count = u16_at(&read_array::<2>(reader)?);
    *at = ifd + 2;
    for _ in 0..count {
        if *at + ENTRY_LEN > len {
            return Ok(None);
        }
        let entry: [u8; 12] = read_array(reader)?;
        *at += ENTRY_LEN;
        if u16_at(&entry[..2]) == ORIENTATION_TAG {
            // A SHORT, in the first two bytes of the value field.
            return Ok(Some(u16_at(&entry[8..10])));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::test_support::tiff;

    fn read(data: &[u8], len: u64) -> (u32, Vec<u8>) {
        let mut reader = BufReader::new(Cursor::new([data, b"NEXT"].concat()));
        let orientation = read_orientation(&mut reader, len).unwrap();
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        (orientation, rest)
    }

    #[test]
    fn reads_the_orientation_and_skips_the_rest() {
        let data = tiff(6);
        assert_eq!(read(&data, data.len() as u64), (6, b"NEXT".to_vec()));
    }

    #[test]
    fn reads_little_endian() {
        let mut data = b"II*\0\x08\0\0\0\x01\0".to_vec();
        data.extend([0x12, 0x01, 3, 0, 1, 0, 0, 0, 8, 0, 0, 0]);
        assert_eq!(read(&data, data.len() as u64).0, 8);
    }

    #[test]
    fn falls_back_to_upright() {
        let data = tiff(6);
        // The entry runs past the structure.
        assert_eq!(read(&data, 25), (UPRIGHT, [&data[25..], b"NEXT"].concat()));
        assert_eq!(read(b"not a tiff", 10).0, UPRIGHT);
        assert_eq!(read(&tiff(9), 38).0, UPRIGHT);
    }

    #[test]
    fn swaps_sides_for_quarter_turns() {
        assert_eq!(upright((4, 3), 1), (4, 3));
        assert_eq!(upright((4, 3), 3), (4, 3));
        assert_eq!(upright((4, 3), 6), (3, 4));
        assert_eq!(upright((4, 3), 8), (3, 4));
    }
}
