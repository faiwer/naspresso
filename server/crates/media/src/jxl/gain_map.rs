//! JPEG XL gain maps: the `jhgm` box (ISO/IEC 18181-2), holding ISO 21496-1
//! metadata and the gain map as a bare codestream.

use fast_image_resize::images::Image;

use super::decode::{DecodeError, decode_native};
use crate::gain_map::GainMapMetadata;

/// The base image and gain map of a JPEG XL file, both upright.
pub struct GainMapped {
    pub metadata: GainMapMetadata,
    /// 8-bit RGB in its own color space.
    pub base: Image<'static>,
    /// The base image's color space, which the gain map applies in.
    pub icc: Vec<u8>,
    /// 8-bit RGB.
    pub gain_map: Image<'static>,
}

/// The parts of a `jhgm` box.
struct GainMapBox<'a> {
    metadata: &'a [u8],
    codestream: &'a [u8],
}

/// The JPEG XL `file`'s base image and gain map. Fails if it has no gain map.
pub fn decode_gain_mapped(file: &[u8]) -> Result<GainMapped, DecodeError> {
    let found = find_box(file, *b"jhgm")
        .and_then(parse_box)
        .ok_or_else(|| DecodeError::new("no readable jhgm box"))?;
    let metadata = GainMapMetadata::from_iso(found.metadata)
        .ok_or_else(|| DecodeError::new("unreadable gain map metadata"))?;
    let (base, icc) = decode_native(file)?;
    let (gain_map, _) = decode_native(found.codestream)?;
    Ok(GainMapped {
        metadata,
        base,
        icc,
        gain_map,
    })
}

/// The body of the first box of `kind` in a container `file`.
fn find_box(file: &[u8], kind: [u8; 4]) -> Option<&[u8]> {
    let mut rest = file;
    while rest.len() >= 8 {
        let size = u32::from_be_bytes(rest[..4].try_into().ok()?);
        let (header_len, size) = match size {
            0 => (8, rest.len()),
            1 => (
                16,
                usize::try_from(u64::from_be_bytes(rest.get(8..16)?.try_into().ok()?)).ok()?,
            ),
            size => (8, usize::try_from(size).ok()?),
        };
        let body = rest.get(header_len..size)?;
        if rest[4..8] == kind {
            return Some(body);
        }
        rest = &rest[size..];
    }
    None
}

/// Version 0: the metadata, an optional color encoding and ICC profile of the
/// alternate image, then the codestream.
fn parse_box(body: &[u8]) -> Option<GainMapBox<'_>> {
    let (&version, rest) = body.split_first()?;
    if version != 0 {
        return None;
    }
    let (metadata, rest) = split_sized(rest, 2)?;
    let (_color_encoding, rest) = split_sized(rest, 1)?;
    let (_icc, codestream) = split_sized(rest, 4)?;
    Some(GainMapBox {
        metadata,
        codestream,
    })
}

/// A field after its big-endian length of `len_bytes`, and what follows it.
fn split_sized(data: &[u8], len_bytes: usize) -> Option<(&[u8], &[u8])> {
    let (len, rest) = data.split_at_checked(len_bytes)?;
    let len = len
        .iter()
        .fold(0usize, |len, &byte| len << 8 | usize::from(byte));
    rest.split_at_checked(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jxl_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let len = u32::try_from(8 + body.len()).unwrap();
        [&len.to_be_bytes()[..], kind, body].concat()
    }

    #[test]
    fn finds_the_box_between_codestream_parts() {
        let file = [
            jxl_box(b"JXL ", b"\r\n\x87\n"),
            jxl_box(b"jxlp", b"FIRST"),
            jxl_box(b"jhgm", b"GAIN"),
            jxl_box(b"jxlp", b"SECOND"),
        ]
        .concat();
        assert_eq!(find_box(&file, *b"jhgm"), Some(&b"GAIN"[..]));
        assert_eq!(find_box(&file, *b"jbrd"), None);
    }

    #[test]
    fn splits_the_box() {
        let body = [
            &[0, 0, 4][..],
            b"META",
            &[2, 0xAA, 0xBB],
            &[0, 0, 0, 3],
            b"ICC",
            b"CODESTREAM",
        ]
        .concat();
        let parts = parse_box(&body).unwrap();
        assert_eq!(parts.metadata, b"META");
        assert_eq!(parts.codestream, b"CODESTREAM");
    }

    #[test]
    fn rejects_other_versions_and_truncated_boxes() {
        assert!(parse_box(&[1, 0, 0, 0, 0, 0, 0, 0, 0]).is_none());
        assert!(parse_box(&[0, 0, 9, b'M']).is_none());
    }
}
