//! HEIF and AVIF: size, rotation and HDR kind from the `meta` box, which is
//! read whole; whether it's a sequence from the `ftyp` brands. The boxes before
//! it are skipped. Encoders put `meta` near the start.

use std::io::{self, BufReader, Read, Seek};

use crate::{
    bytes::{contains, invalid_data, read_array, read_u32, read_u64, skip},
    cicp::hdr_from_transfer,
    metadata::{Hdr, ImageMetadata},
};

/// Real `meta` boxes take a few KiB; this keeps a malformed one from making
/// us read much.
const MAX_META_LEN: u64 = 1 << 20;

pub(super) type BoxType = [u8; 4];

/// Expects the reader at the start of the file.
pub fn read<R: Read + Seek>(reader: &mut BufReader<R>) -> io::Result<ImageMetadata> {
    let mut animated = false;
    loop {
        let (kind, body_len) = box_header(reader)?;
        match &kind {
            b"ftyp" if body_len <= MAX_META_LEN => {
                let mut ftyp = vec![0; body_len as usize];
                reader.read_exact(&mut ftyp)?;
                animated = is_sequence(&ftyp);
            }
            b"meta" => {
                if body_len > MAX_META_LEN {
                    return Err(invalid_data("meta box too large"));
                }
                let mut meta = vec![0; body_len as usize];
                reader.read_exact(&mut meta)?;
                let mut image = parse_meta(&meta)
                    .ok_or_else(|| invalid_data("no primary image size in meta"))?;
                image.animated = animated;
                return Ok(image);
            }
            _ => skip(reader, body_len)?,
        }
    }
}

/// Whether the `ftyp` body names an image sequence brand: the major brand, a
/// minor version, then the compatible brands.
fn is_sequence(ftyp: &[u8]) -> bool {
    const SEQUENCE_BRANDS: [&[u8; 4]; 3] = [b"avis", b"msf1", b"hevs"];
    ftyp.get(..4)
        .into_iter()
        .chain(
            ftyp.get(8..)
                .into_iter()
                .flat_map(|rest| rest.chunks_exact(4)),
        )
        .any(|brand| SEQUENCE_BRANDS.iter().any(|known| brand == *known))
}

/// The type and body length of the box at the reader.
fn box_header(reader: &mut impl Read) -> io::Result<(BoxType, u64)> {
    let size = read_u32(reader)?;
    let kind = read_array(reader)?;
    let (header_len, size) = match size {
        // Runs to the end of the file, so `meta` isn't after it.
        0 => return Err(invalid_data("no meta box")),
        1 => (16, read_u64(reader)?),
        size => (8, size.into()),
    };
    let body_len = size
        .checked_sub(header_len)
        .ok_or_else(|| invalid_data("box shorter than its header"))?;
    Ok((kind, body_len))
}

fn parse_meta(meta: &[u8]) -> Option<ImageMetadata> {
    let mut primary = None;
    let mut gain_map = false;
    let mut properties = Vec::new();
    let mut associations = Vec::new();

    // `meta` is a full box: version and flags come before the children.
    for (kind, body) in boxes(meta.get(4..)?) {
        match &kind {
            b"pitm" => primary = parse_pitm(body),
            b"iinf" => gain_map |= has_tone_map_item(body),
            b"iprp" => {
                for (kind, body) in boxes(body) {
                    match &kind {
                        b"ipco" => properties = boxes(body).collect(),
                        b"ipma" => associations = parse_ipma(body)?,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // Apple marks its gain map as an auxiliary image.
    gain_map |= properties
        .iter()
        .any(|(kind, body)| kind == b"auxC" && contains(body, b"hdrgainmap"));

    let primary = primary?;
    let (_, indices) = associations
        .into_iter()
        .find(|(item, _)| *item == primary)?;
    // Property indices are 1-based; 0 means none.
    let primary_properties: Vec<_> = indices
        .into_iter()
        .filter_map(|index| properties.get(usize::from(index).checked_sub(1)?))
        .collect();

    let (width, height) = primary_properties.iter().find_map(|(kind, body)| {
        if kind == b"ispe" {
            parse_ispe(body)
        } else {
            None
        }
    })?;
    // Counter-clockwise quarter turns, applied after decoding; `ispe` is the size before.
    let quarter_turns = primary_properties
        .iter()
        .find_map(|(kind, body)| (kind == b"irot").then(|| body.first().map(|angle| angle & 3)))
        .flatten()
        .unwrap_or(0);
    let (width, height) = if quarter_turns % 2 == 1 {
        (height, width)
    } else {
        (width, height)
    };
    let hdr = primary_properties
        .iter()
        .find_map(|(kind, body)| {
            if kind == b"colr" {
                nclx_transfer(body)
            } else {
                None
            }
        })
        .map_or(Hdr::None, hdr_from_transfer);
    // An HDR base image's gain map is only for SDR displays.
    let hdr = match hdr {
        Hdr::None if gain_map => Hdr::GainMap,
        hdr => hdr,
    };

    Some(ImageMetadata {
        width,
        height,
        hdr,
        animated: false,
    })
}

/// The `(type, body)` of each box in `data`, up to the first malformed one.
pub(super) fn boxes(mut data: &[u8]) -> impl Iterator<Item = (BoxType, &[u8])> {
    std::iter::from_fn(move || {
        let size = be_u32(data, 0)?;
        let kind = data.get(4..8)?.try_into().ok()?;
        let (header_len, size) = match size {
            0 => (8, data.len()),
            1 => (16, usize::try_from(be_u64(data, 8)?).ok()?),
            size => (8, usize::try_from(size).ok()?),
        };
        let body = data.get(header_len..size)?;
        data = &data[size..];
        Some((kind, body))
    })
}

/// The primary item's ID.
pub(super) fn parse_pitm(body: &[u8]) -> Option<u32> {
    match body.first()? {
        0 => be_u16(body, 4).map(u32::from),
        _ => be_u32(body, 4),
    }
}

/// Whether the item list has a `tmap` item: an ISO 21496-1 gain map.
fn has_tone_map_item(iinf: &[u8]) -> bool {
    let entries_at = match iinf.first() {
        Some(0) => 6,
        Some(_) => 8,
        None => return false,
    };
    iinf.get(entries_at..).is_some_and(|entries| {
        boxes(entries).any(|(kind, body)| &kind == b"infe" && item_type(body) == Some(*b"tmap"))
    })
}

/// The type of an `infe` item; versions before 2 have none.
fn item_type(infe: &[u8]) -> Option<BoxType> {
    let id_len = match infe.first()? {
        2 => 2,
        3 => 4,
        _ => return None,
    };
    let at = 4 + id_len + 2;
    infe.get(at..at + 4)?.try_into().ok()
}

/// Each item's ID and its 1-based property indices.
pub(super) fn parse_ipma(body: &[u8]) -> Option<Vec<(u32, Vec<u16>)>> {
    let version = *body.first()?;
    let wide_indices = body.get(3)? & 1 == 1;
    let count = be_u32(body, 4)?;
    let mut at = 8;
    let mut items = Vec::new();
    for _ in 0..count {
        let item = if version == 0 {
            at += 2;
            be_u16(body, at - 2)?.into()
        } else {
            at += 4;
            be_u32(body, at - 4)?
        };
        let association_count = *body.get(at)?;
        at += 1;
        let mut indices = Vec::with_capacity(association_count.into());
        for _ in 0..association_count {
            // The top bit flags the property as essential.
            let index = if wide_indices {
                at += 2;
                be_u16(body, at - 2)? & 0x7FFF
            } else {
                at += 1;
                u16::from(body.get(at - 1)? & 0x7F)
            };
            indices.push(index);
        }
        items.push((item, indices));
    }
    Some(items)
}

fn parse_ispe(body: &[u8]) -> Option<(u32, u32)> {
    let size = (be_u32(body, 4)?, be_u32(body, 8)?);
    (size.0 > 0 && size.1 > 0).then_some(size)
}

/// The transfer characteristics of an `nclx` color box. ICC-profile boxes
/// don't signal one.
fn nclx_transfer(colr: &[u8]) -> Option<u16> {
    if colr.get(..4)? != b"nclx" {
        return None;
    }
    be_u16(colr, 6)
}

pub(super) fn be_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

pub(super) fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

pub(super) fn be_u64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::page_reader;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let size = u32::try_from(body.len() + 8).unwrap();
        [&size.to_be_bytes()[..], kind, body].concat()
    }

    /// A full box: version, then zeroed flags but for the low byte.
    fn full(kind: &[u8; 4], version: u8, flags: u8, body: &[u8]) -> Vec<u8> {
        bx(kind, &[&[version, 0, 0, flags][..], body].concat())
    }

    fn infe(id: u16, item_type: &[u8; 4]) -> Vec<u8> {
        full(
            b"infe",
            2,
            0,
            &[&id.to_be_bytes()[..], &[0, 0], item_type, b"\0"].concat(),
        )
    }

    fn ispe(width: u32, height: u32) -> Vec<u8> {
        full(
            b"ispe",
            0,
            0,
            &[width.to_be_bytes(), height.to_be_bytes()].concat(),
        )
    }

    fn nclx(transfer: u16) -> Vec<u8> {
        bx(
            b"colr",
            &[
                &b"nclx"[..],
                &9u16.to_be_bytes(),
                &transfer.to_be_bytes(),
                &[0, 9, 0x80],
            ]
            .concat(),
        )
    }

    struct Fixture {
        items: Vec<Vec<u8>>,
        properties: Vec<Vec<u8>>,
        /// The primary item's 1-based property indices.
        primary_properties: Vec<u8>,
    }

    impl Fixture {
        fn avif(properties: Vec<Vec<u8>>) -> Self {
            let primary_properties = (1..=properties.len() as u8).collect();
            Self {
                items: vec![infe(1, b"av01")],
                properties,
                primary_properties,
            }
        }

        fn build(self) -> Vec<u8> {
            let item_count = u16::try_from(self.items.len()).unwrap();
            let iinf = full(
                b"iinf",
                0,
                0,
                &[item_count.to_be_bytes().to_vec(), self.items.concat()].concat(),
            );
            let ipco = bx(b"ipco", &self.properties.concat());
            let mut ipma = vec![0, 0, 0, 1, 0, 1, self.primary_properties.len() as u8];
            ipma.extend_from_slice(&self.primary_properties);
            let iprp = bx(b"iprp", &[ipco, full(b"ipma", 0, 0, &ipma)].concat());
            let meta = full(
                b"meta",
                0,
                0,
                &[
                    full(b"hdlr", 0, 0, &[0; 21]),
                    full(b"pitm", 0, 0, &[0, 1]),
                    iinf,
                    iprp,
                ]
                .concat(),
            );
            [
                bx(b"ftyp", b"avifmif1miaf"),
                meta,
                bx(b"mdat", &[0xAB; 5000]),
            ]
            .concat()
        }
    }

    fn probe(fixture: Fixture) -> ImageMetadata {
        let (mut reader, _) = page_reader(fixture.build());
        read(&mut reader).unwrap()
    }

    #[test]
    fn finds_sequence_brands() {
        assert!(is_sequence(b"avis\0\0\0\0avifmif1"));
        assert!(is_sequence(b"heic\0\0\0\0mif1msf1"));
        assert!(!is_sequence(b"avif\0\0\0\0avifmif1miafMA1A"));
        // The minor version isn't a brand.
        assert!(!is_sequence(b"heicavismif1"));
    }

    #[test]
    fn reads_the_primary_size() {
        let meta = probe(Fixture::avif(vec![ispe(4032, 3024)]));
        assert!(!meta.animated);
        assert_eq!((meta.width, meta.height, meta.hdr), (4032, 3024, Hdr::None));
    }

    #[test]
    fn turns_the_size_by_irot() {
        let quarter = probe(Fixture::avif(vec![ispe(4032, 3024), bx(b"irot", &[1])]));
        assert_eq!((quarter.width, quarter.height), (3024, 4032));
        let half = probe(Fixture::avif(vec![ispe(4032, 3024), bx(b"irot", &[2])]));
        assert_eq!((half.width, half.height), (4032, 3024));
    }

    #[test]
    fn reads_the_transfer_from_nclx() {
        assert_eq!(
            probe(Fixture::avif(vec![ispe(8, 8), nclx(16)])).hdr,
            Hdr::Pq
        );
        assert_eq!(
            probe(Fixture::avif(vec![ispe(8, 8), nclx(18)])).hdr,
            Hdr::Hlg
        );
        assert_eq!(
            probe(Fixture::avif(vec![ispe(8, 8), nclx(13)])).hdr,
            Hdr::None
        );
    }

    #[test]
    fn a_tone_map_item_means_a_gain_map() {
        let mut fixture = Fixture::avif(vec![ispe(8, 8), nclx(13)]);
        fixture.items.push(infe(2, b"tmap"));
        assert_eq!(probe(fixture).hdr, Hdr::GainMap);
    }

    #[test]
    fn a_pq_base_image_outranks_its_gain_map() {
        let mut fixture = Fixture::avif(vec![ispe(8, 8), nclx(16)]);
        fixture.items.push(infe(2, b"tmap"));
        assert_eq!(probe(fixture).hdr, Hdr::Pq);
    }

    #[test]
    fn an_apple_gain_map_auxiliary_means_a_gain_map() {
        let aux = full(b"auxC", 0, 0, b"urn:com:apple:photo:2020:aux:hdrgainmap\0");
        let mut fixture = Fixture::avif(vec![ispe(8, 8), aux]);
        fixture.primary_properties = vec![1];
        assert_eq!(probe(fixture).hdr, Hdr::GainMap);
    }

    #[test]
    fn ignores_properties_of_other_items() {
        let mut fixture = Fixture::avif(vec![ispe(8, 8), nclx(16), ispe(100, 100)]);
        fixture.primary_properties = vec![3];
        let meta = probe(fixture);
        assert_eq!((meta.width, meta.height, meta.hdr), (100, 100, Hdr::None));
    }

    #[test]
    fn fails_without_a_primary_size() {
        let (mut reader, _) = page_reader(Fixture::avif(vec![nclx(16)]).build());
        assert!(read(&mut reader).is_err());
    }
}
