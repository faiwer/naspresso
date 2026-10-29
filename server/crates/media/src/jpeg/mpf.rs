//! Gain-map JPEGs (Ultra HDR, ISO 21496-1): the base image, then the gain map
//! as a second JPEG appended after it, located by the base's MPF segment.

use std::ops::Range;

use super::markers::{APP1, APP2, EOI, ISO_GAIN_MAP_ID, SOI, SOS, XMP_ID, is_standalone};
use crate::gain_map::GainMapMetadata;

const MPF_ID: &[u8] = b"MPF\0";
/// The MPF tag listing the images, 16 bytes each.
const MP_ENTRY_TAG: u16 = 0xB002;
const MP_ENTRY_LEN: usize = 16;

pub struct GainMapJpeg<'a> {
    /// The gain map, a JPEG of its own.
    pub jpeg: &'a [u8],
    pub metadata: GainMapMetadata,
}

/// The gain map of `jpeg`. `None` when there's none, or none we can read.
pub fn find_gain_map(jpeg: &[u8]) -> Option<GainMapJpeg<'_>> {
    let (start, mpf) = segments(jpeg)?.find_map(|segment| {
        let mpf = segment.body.strip_prefix(MPF_ID)?;
        Some((segment.start + MPF_ID.len(), mpf))
    })?;
    let range = second_image(mpf, start)?;
    let gain_map = jpeg.get(range)?;
    Some(GainMapJpeg {
        jpeg: gain_map,
        metadata: metadata(gain_map)?,
    })
}

/// From the ISO block, which is exact, else from the XMP.
fn metadata(gain_map: &[u8]) -> Option<GainMapMetadata> {
    let mut xmp = None;
    for segment in segments(gain_map)? {
        match segment.marker {
            APP2 => {
                if let Some(iso) = segment.body.strip_prefix(ISO_GAIN_MAP_ID)
                    && let Some(metadata) = GainMapMetadata::from_iso(iso)
                {
                    return Some(metadata);
                }
            }
            APP1 => xmp = xmp.or_else(|| segment.body.strip_prefix(XMP_ID)),
            _ => {}
        }
    }
    GainMapMetadata::from_xmp(xmp?)
}

/// Where the second image of an MPF index is in the file. `tiff_start` is the
/// file offset of the index, which its image offsets are relative to.
fn second_image(mpf: &[u8], tiff_start: usize) -> Option<Range<usize>> {
    let order = match mpf.get(..4)? {
        b"MM\0\x2A" => ByteOrder::Big,
        b"II\x2A\0" => ByteOrder::Little,
        _ => return None,
    };
    let ifd = order.u32(mpf, 4)? as usize;
    let count = order.u16(mpf, ifd)?;
    let (entries_len, entries_offset) = (0..usize::from(count)).find_map(|i| {
        let entry = ifd + 2 + i * 12;
        (order.u16(mpf, entry)? == MP_ENTRY_TAG)
            .then(|| Some((order.u32(mpf, entry + 4)?, order.u32(mpf, entry + 8)?)))?
    })?;
    if (entries_len as usize) < 2 * MP_ENTRY_LEN {
        return None;
    }

    let second = entries_offset as usize + MP_ENTRY_LEN;
    let size = order.u32(mpf, second + 4)? as usize;
    let offset = order.u32(mpf, second + 8)? as usize;
    let start = tiff_start.checked_add(offset)?;
    Some(start..start.checked_add(size)?)
}

enum ByteOrder {
    Big,
    Little,
}

impl ByteOrder {
    fn u16(&self, data: &[u8], at: usize) -> Option<u16> {
        let bytes = *data.get(at..)?.first_chunk()?;
        Some(match self {
            ByteOrder::Big => u16::from_be_bytes(bytes),
            ByteOrder::Little => u16::from_le_bytes(bytes),
        })
    }

    fn u32(&self, data: &[u8], at: usize) -> Option<u32> {
        let bytes = *data.get(at..)?.first_chunk()?;
        Some(match self {
            ByteOrder::Big => u32::from_be_bytes(bytes),
            ByteOrder::Little => u32::from_le_bytes(bytes),
        })
    }
}

struct Segment<'a> {
    marker: u8,
    /// File offset of `body`.
    start: usize,
    body: &'a [u8],
}

/// The segments before the image data. `None` if `jpeg` isn't a JPEG.
fn segments(jpeg: &[u8]) -> Option<impl Iterator<Item = Segment<'_>>> {
    if jpeg.get(..2)? != [0xFF, SOI] {
        return None;
    }
    let mut at = 2;
    Some(std::iter::from_fn(move || {
        loop {
            if *jpeg.get(at)? != 0xFF {
                return None;
            }
            // Any number of 0xFF fill bytes may precede the marker code.
            while *jpeg.get(at + 1)? == 0xFF {
                at += 1;
            }
            let marker = jpeg[at + 1];
            at += 2;
            if is_standalone(marker) {
                continue;
            }
            if marker == SOS || marker == EOI {
                return None;
            }
            let len = usize::from(u16::from_be_bytes(*jpeg.get(at..)?.first_chunk()?));
            let start = at + 2;
            let end = start + len.checked_sub(2)?;
            let body = jpeg.get(start..end)?;
            at = end;
            return Some(Segment {
                marker,
                start,
                body,
            });
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
        let len = u16::try_from(body.len() + 2).unwrap();
        [&[0xFF, marker][..], &len.to_be_bytes(), body].concat()
    }

    fn jpeg(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut data = vec![0xFF, SOI];
        for segment in segments {
            data.extend_from_slice(segment);
        }
        data.extend_from_slice(&[0xFF, SOS, 0, 2, 0xAB, 0xAB, 0xFF, EOI]);
        data
    }

    /// An MPF segment listing a primary image and a second one at `offset`
    /// (relative to the index) of `size` bytes.
    fn mpf(size: u32, offset: u32) -> Vec<u8> {
        let mut tiff = b"MM\0\x2A".to_vec();
        tiff.extend_from_slice(&8u32.to_be_bytes());
        tiff.extend_from_slice(&1u16.to_be_bytes());
        tiff.extend_from_slice(&MP_ENTRY_TAG.to_be_bytes());
        tiff.extend_from_slice(&7u16.to_be_bytes());
        tiff.extend_from_slice(&32u32.to_be_bytes());
        tiff.extend_from_slice(&26u32.to_be_bytes());
        tiff.extend_from_slice(&0u32.to_be_bytes());
        tiff.extend_from_slice(&[0; 16]);
        tiff.extend_from_slice(&[0; 4]);
        tiff.extend_from_slice(&size.to_be_bytes());
        tiff.extend_from_slice(&offset.to_be_bytes());
        tiff.extend_from_slice(&[0; 4]);
        segment(APP2, &[MPF_ID, &tiff].concat())
    }

    fn xmp_gain_map() -> Vec<u8> {
        let xmp = br#"<x:xmpmeta hdrgm:Version="1.0" hdrgm:GainMapMax="2.5"/>"#;
        jpeg(&[segment(APP1, &[XMP_ID, xmp].concat())])
    }

    /// A base JPEG with `gain_map` appended and indexed by MPF.
    fn ultra_hdr(gain_map: &[u8]) -> Vec<u8> {
        let placeholder = jpeg(&[mpf(0, 0)]);
        // The MPF index starts after SOI, the APP2 marker and length, and "MPF\0".
        let tiff_start = 2 + 4 + MPF_ID.len();
        let offset = u32::try_from(placeholder.len() - tiff_start).unwrap();
        let size = u32::try_from(gain_map.len()).unwrap();
        [jpeg(&[mpf(size, offset)]), gain_map.to_vec()].concat()
    }

    #[test]
    fn finds_the_appended_gain_map() {
        let gain_map = xmp_gain_map();
        let file = ultra_hdr(&gain_map);
        let found = find_gain_map(&file).unwrap();
        assert_eq!(found.jpeg, gain_map);
        assert_eq!(found.metadata.channels[0].gain_map_max.n, 2_500_000);
    }

    #[test]
    fn prefers_iso_metadata_over_xmp() {
        let mut iso = ISO_GAIN_MAP_ID.to_vec();
        iso.extend_from_slice(&[0, 0, 0, 0, 0x40]);
        for (n, d) in [(0u32, 1u32), (3, 1)] {
            iso.extend_from_slice(&n.to_be_bytes());
            iso.extend_from_slice(&d.to_be_bytes());
        }
        for (n, d) in [(0i32, 1u32), (3, 1), (1, 1), (1, 64), (1, 64)] {
            iso.extend_from_slice(&n.to_be_bytes());
            iso.extend_from_slice(&d.to_be_bytes());
        }
        let xmp = br#"hdrgm:Version="1.0" hdrgm:GainMapMax="2.5""#;
        let gain_map = jpeg(&[segment(APP1, &[XMP_ID, xmp].concat()), segment(APP2, &iso)]);

        let file = ultra_hdr(&gain_map);
        let max = find_gain_map(&file).unwrap().metadata.channels[2].gain_map_max;
        assert_eq!((max.n, max.d), (3, 1));
    }

    #[test]
    fn needs_an_index_and_metadata() {
        assert!(find_gain_map(&jpeg(&[])).is_none());
        let no_metadata = jpeg(&[]);
        assert!(find_gain_map(&ultra_hdr(&no_metadata)).is_none());
        let mut truncated = ultra_hdr(&xmp_gain_map());
        truncated.truncate(truncated.len() - 1);
        assert!(find_gain_map(&truncated).is_none());
    }
}
