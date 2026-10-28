//! JPEG: size from the frame header, gain-map HDR from the segments before it.
//! Segment bodies are skipped, except XMP, which is read until a match.

use std::io::{self, BufReader, Read, Seek};

use super::bytes::{contains_any, invalid_data, read_array, read_u8, read_u16, skip};
use crate::storage::{Hdr, ImageMetadata};

const SOI: u8 = 0xD8;
const EOI: u8 = 0xD9;
const SOS: u8 = 0xDA;
const APP1: u8 = 0xE1;
const APP2: u8 = 0xE2;

/// Opens an XMP APP1 segment.
const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// Opens an ISO 21496-1 gain-map APP2 segment.
const ISO_GAIN_MAP_ID: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";
/// XMP properties of Ultra HDR and of Apple's HDR JPEGs.
const GAIN_MAP_XMP: [&[u8]; 2] = [b"hdrgm:Version", b"HDRGainMapVersion"];

/// Expects the reader at the start of the file.
pub fn read<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<ImageMetadata> {
    if read_array(reader)? != [0xFF, SOI] {
        return Err(invalid_data("not a JPEG"));
    }

    let mut gain_map = false;
    loop {
        let marker = next_marker(reader)?;
        if is_standalone(marker) {
            continue;
        }
        if marker == SOS || marker == EOI {
            return Err(invalid_data("no frame header before the image data"));
        }
        let body = u64::from(read_u16(reader)?)
            .checked_sub(2)
            .ok_or_else(|| invalid_data("segment shorter than its length field"))?;

        match marker {
            _ if is_frame_header(marker) => {
                let [_precision, h1, h0, w1, w0] = read_array(reader)?;
                let height = u16::from_be_bytes([h1, h0]);
                let width = u16::from_be_bytes([w1, w0]);
                // A zero height is defined later by a DNL segment; too rare to chase.
                if width == 0 || height == 0 {
                    return Err(invalid_data("no size in the frame header"));
                }
                return Ok(ImageMetadata {
                    width: width.into(),
                    height: height.into(),
                    hdr: if gain_map { Hdr::GainMap } else { Hdr::None },
                });
            }
            APP1 => {
                if let Some(rest) = enter_if(reader, body, XMP_ID)? {
                    gain_map |= contains_any(reader, rest, &GAIN_MAP_XMP)?;
                }
            }
            APP2 => {
                if let Some(rest) = enter_if(reader, body, ISO_GAIN_MAP_ID)? {
                    gain_map = true;
                    skip(reader, rest)?;
                }
            }
            _ => skip(reader, body)?,
        }
    }
}

fn next_marker(reader: &mut impl Read) -> io::Result<u8> {
    if read_u8(reader)? != 0xFF {
        return Err(invalid_data("expected a marker"));
    }
    // Any number of 0xFF fill bytes may precede the marker code.
    loop {
        let code = read_u8(reader)?;
        if code != 0xFF {
            return Ok(code);
        }
    }
}

/// Markers without a length or body.
fn is_standalone(marker: u8) -> bool {
    matches!(marker, 0x01 | 0xD0..=0xD7 | SOI)
}

/// SOF0 to SOF15, minus the Huffman (C4), arithmetic (CC) and reserved (C8)
/// markers that share the range.
fn is_frame_header(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

/// Reads a segment body's leading identifier. When it's `id`, returns the
/// bytes left in the body, unread; otherwise skips the whole body.
fn enter_if<R: Read + Seek>(
    reader: &mut BufReader<R>,
    body: u64,
    id: &[u8],
) -> io::Result<Option<u64>> {
    let id_len = id.len() as u64;
    if body < id_len {
        skip(reader, body)?;
        return Ok(None);
    }
    let mut prefix = vec![0; id.len()];
    reader.read_exact(&mut prefix)?;
    let rest = body - id_len;
    if prefix == id {
        Ok(Some(rest))
    } else {
        skip(reader, rest)?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::media::test_support::page_reader;

    fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
        let len = u16::try_from(body.len() + 2).unwrap();
        [&[0xFF, marker][..], &len.to_be_bytes(), body].concat()
    }

    fn frame_header(width: u16, height: u16) -> Vec<u8> {
        let mut body = vec![8];
        body.extend_from_slice(&height.to_be_bytes());
        body.extend_from_slice(&width.to_be_bytes());
        body.extend_from_slice(&[1, 1, 0x11, 0]);
        segment(0xC0, &body)
    }

    fn jpeg(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut data = vec![0xFF, SOI];
        for segment in segments {
            data.extend_from_slice(segment);
        }
        data.extend_from_slice(&[0xFF, SOS, 0, 2]);
        data.extend_from_slice(&[0xAB; 1000]);
        data
    }

    /// A phone-sized EXIF block, mostly a thumbnail.
    fn exif() -> Vec<u8> {
        let mut body = b"Exif\0\0".to_vec();
        body.resize(60_000, 0x55);
        segment(APP1, &body)
    }

    fn xmp(properties: &str) -> Vec<u8> {
        let body = [XMP_ID, b"<x:xmpmeta ", properties.as_bytes(), b"/>"].concat();
        segment(APP1, &body)
    }

    #[test]
    fn reads_the_size_of_a_plain_jpeg() {
        let (mut reader, _) = page_reader(jpeg(&[frame_header(640, 480)]));
        let meta = read(&mut reader).unwrap();
        assert_eq!((meta.width, meta.height, meta.hdr), (640, 480, Hdr::None));
    }

    #[test]
    fn skips_exif_instead_of_reading_it() {
        let data = jpeg(&[exif(), frame_header(4032, 3024)]);
        let (mut reader, read_bytes) = page_reader(data);

        let meta = read(&mut reader).unwrap();
        assert_eq!((meta.width, meta.height), (4032, 3024));
        // The first page, then the page at the frame header.
        assert!(
            read_bytes.get() <= 2 * 4096,
            "read {} bytes",
            read_bytes.get()
        );
    }

    #[test]
    fn finds_an_ultra_hdr_gain_map_in_xmp() {
        let data = jpeg(&[
            exif(),
            xmp(r#"hdrgm:Version="1.0""#),
            frame_header(4000, 3000),
        ]);
        let (mut reader, _) = page_reader(data);
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::GainMap);
    }

    #[test]
    fn finds_an_apple_gain_map_in_xmp() {
        let data = jpeg(&[
            xmp(r#"HDRGainMap:HDRGainMapVersion="65536""#),
            frame_header(8, 8),
        ]);
        let (mut reader, _) = page_reader(data);
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::GainMap);
    }

    #[test]
    fn finds_an_iso_gain_map_segment() {
        let iso = segment(APP2, &[ISO_GAIN_MAP_ID, &[0, 0, 0, 0]].concat());
        let (mut reader, _) = page_reader(jpeg(&[iso, frame_header(8, 8)]));
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::GainMap);
    }

    #[test]
    fn ignores_xmp_without_gain_map_properties() {
        let data = jpeg(&[xmp(r#"xmp:Rating="5""#), frame_header(8, 8)]);
        let (mut reader, _) = page_reader(data);
        assert_eq!(read(&mut reader).unwrap().hdr, Hdr::None);
    }

    #[test]
    fn rejects_non_jpegs_and_truncated_files() {
        let (mut reader, _) = page_reader(b"GIF89a".to_vec());
        assert!(read(&mut reader).is_err());

        let mut truncated = jpeg(&[exif(), frame_header(8, 8)]);
        truncated.truncate(10_000);
        let (mut reader, _) = page_reader(truncated);
        assert!(read(&mut reader).is_err());
    }
}
