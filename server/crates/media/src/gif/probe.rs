//! GIF: whether it's animated. A loop extension, which comes before the first
//! frame, says so early. Without one, an animation plays once, and only a
//! second frame tells: frame data is skipped, but in blocks of at most 255
//! bytes, so reaching it reads about as much as the first frame takes.

use std::io::{self, BufReader, Read, Seek};

use crate::bytes::{invalid_data, read_array, read_u8, skip};

const EXTENSION: u8 = 0x21;
const IMAGE: u8 = 0x2C;
const TRAILER: u8 = 0x3B;
const HAS_COLOR_TABLE: u8 = 0x80;
const APPLICATION: u8 = 0xFF;
const LOOP_APPS: [&[u8; 11]; 2] = [b"NETSCAPE2.0", b"ANIMEXTS1.0"];

/// Expects the reader at the start of the file.
pub fn read_animated<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<bool> {
    let header: [u8; 13] = read_array(reader)?;
    if !matches!(&header[..6], b"GIF87a" | b"GIF89a") {
        return Err(invalid_data("not a GIF"));
    }
    skip(reader, color_table_len(header[10]))?;
    let mut frames = 0;
    loop {
        let block = match read_u8(reader) {
            Ok(block) => block,
            // Truncated: what's there plays as it is.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(e),
        };
        match block {
            EXTENSION => {
                let label = read_u8(reader)?;
                let len = read_u8(reader)?;
                if label == APPLICATION && len == 11 {
                    let id: [u8; 11] = read_array(reader)?;
                    if LOOP_APPS.contains(&&id) {
                        return Ok(true);
                    }
                } else {
                    skip(reader, len.into())?;
                }
                if len > 0 {
                    skip_sub_blocks(reader)?;
                }
            }
            IMAGE => {
                frames += 1;
                if frames > 1 {
                    return Ok(true);
                }
                let descriptor: [u8; 9] = read_array(reader)?;
                // The LZW code size comes before the data.
                skip(reader, color_table_len(descriptor[8]) + 1)?;
                skip_sub_blocks(reader)?;
            }
            TRAILER => return Ok(false),
            _ => return Err(invalid_data("unknown GIF block")),
        }
    }
}

/// The bytes of the color table the `flags` byte announces.
fn color_table_len(flags: u8) -> u64 {
    if flags & HAS_COLOR_TABLE == 0 {
        0
    } else {
        3 << ((flags & 0x07) + 1)
    }
}

/// Skips length-prefixed data blocks up to the empty one ending them.
fn skip_sub_blocks<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<()> {
    loop {
        let len = read_u8(reader)?;
        if len == 0 {
            return Ok(());
        }
        skip(reader, len.into())?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SNIFF_LEN, test_support::page_reader};

    /// A GIF with a 4-color global table, a comment, a loop extension if
    /// `looping`, and `frames` frames of two data blocks each.
    fn gif(looping: bool, frames: usize) -> Vec<u8> {
        let mut data = b"GIF89a\x02\0\x02\0\x81\0\0".to_vec();
        data.extend([0; 12]);
        data.extend(b"\x21\xFE\x02hi\0");
        if looping {
            data.extend(b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0\0");
        }
        for _ in 0..frames {
            // A graphic control extension, then the image.
            data.extend(b"\x21\xF9\x04\0\x0A\0\0\0");
            data.extend([IMAGE, 0, 0, 0, 0, 2, 0, 2, 0, 0, 2]);
            data.extend([255].iter().chain(&[0xAB; 255]));
            data.extend([3, 1, 2, 3, 0]);
        }
        data.push(TRAILER);
        data
    }

    fn animated(data: Vec<u8>) -> bool {
        let (mut reader, _) = page_reader(data);
        read_animated(&mut reader).unwrap()
    }

    #[test]
    fn a_loop_extension_means_animated_before_any_frame_data() {
        let mut data = gif(true, 1);
        data.extend(vec![0; 3 * SNIFF_LEN]);
        let (mut reader, read) = page_reader(data);
        assert!(read_animated(&mut reader).unwrap());
        assert_eq!(read.get(), SNIFF_LEN);
    }

    #[test]
    fn a_second_frame_means_animated() {
        assert!(animated(gif(false, 3)));
    }

    #[test]
    fn one_frame_without_a_loop_is_still() {
        assert!(!animated(gif(false, 1)));
    }

    #[test]
    fn other_applications_are_skipped() {
        let mut data = gif(false, 1);
        let at = data.iter().position(|&b| b == IMAGE).unwrap() - 8;
        data.splice(at..at, *b"\x21\xFF\x0BXMP DataXMP\x02ab\0");
        data.splice(at..at, *b"\x21\xFF\0");
        assert!(!animated(data));
    }

    #[test]
    fn skips_a_local_color_table() {
        let mut data = gif(false, 2);
        // The first frame's flags: an 8-color local table, inserted after its descriptor.
        let at = data.iter().position(|&b| b == IMAGE).unwrap();
        data[at + 9] = 0x82;
        data.splice(at + 10..at + 10, [0x2C; 24]);
        assert!(animated(data));
    }

    #[test]
    fn rejects_non_gifs() {
        let (mut reader, _) = page_reader(b"\x89PNG\r\n\x1a\n\0\0\0\0\0".to_vec());
        assert!(read_animated(&mut reader).is_err());
    }
}
