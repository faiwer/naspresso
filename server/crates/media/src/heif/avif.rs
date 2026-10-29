//! AVIF files of two kinds:
//!
//! - Gain-map files (ISO/IEC 23008-12 tone-map derived items): the SDR base
//!   image as the primary item, so older readers still show it, and a `tmap`
//!   item that HDR-capable readers prefer, deriving the HDR rendition from the
//!   base and a hidden gain map.
//! - PQ files: one BT.2100 PQ image, and maybe an alpha item for it.

use crate::{
    av1::{Av1Image, BIT_DEPTH},
    gain_map::GainMapMetadata,
};

const BASE_ID: u16 = 1;
const TONE_MAP_ID: u16 = 2;
const GAIN_MAP_ID: u16 = 3;
/// In PQ files, which have no tone map.
const ALPHA_ID: u16 = 2;
/// Must differ from every item ID.
const ALTERNATIVES_GROUP_ID: u32 = 4;

/// CICP codes for `colr` boxes: primaries, transfer, matrix.
type Cicp = (u16, u16, u16);
/// BT.601: what `av1::encode` codes.
const SDR_MATRIX: u16 = 6;
/// The ICC profile has the primaries and transfer; only the matrix is here.
const MATRIX_ONLY: Cicp = (2, 2, 6);
/// BT.2100 PQ, BT.2020 matrix: what `av1::encode_pq` codes in the bitstream.
const BT2100_PQ: Cicp = (9, 16, 9);
/// Major brand, minor version 0, compatible brands.
const BRANDS: &[u8] = b"avif\0\0\0\0avifmif1miafMA1A";
/// The `auxC` type of an alpha item.
pub const ALPHA_URN: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0";

/// The color space of a gain-map file's base image, which the HDR rendition
/// shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseColor {
    Icc(Vec<u8>),
    Cicp { primaries: u16, transfer: u16 },
}

impl BaseColor {
    pub const SRGB: Self = Self::Cicp {
        primaries: 1,
        transfer: 13,
    };
}

/// `image`, coded by `av1::encode_pq`, as an AVIF, with `alpha`, coded by
/// `av1::encode_alpha`, as its alpha channel if there is one.
pub fn write_pq(image: &Av1Image, alpha: Option<&Av1Image>) -> Vec<u8> {
    let ftyp = plain_box(b"ftyp", BRANDS);
    let meta = |data_start: usize| {
        let mut properties = Properties::default();
        let mut items = vec![(
            BASE_ID,
            vec![
                properties.add(ispe(image), false),
                properties.add(pixi(image), false),
                properties.add(plain_box(b"av1C", &image.config), true),
                properties.add(nclx(BT2100_PQ), false),
            ],
        )];
        let mut locations = vec![(BASE_ID, data_start, image.data.len())];
        let mut infos = vec![(BASE_ID, b"av01", false)];
        let mut body = vec![hdlr(), full_box(b"pitm", 0, 0, &BASE_ID.to_be_bytes())];
        if let Some(alpha) = alpha {
            items.push((
                ALPHA_ID,
                vec![
                    properties.add(ispe(alpha), false),
                    properties.add(pixi(alpha), false),
                    properties.add(plain_box(b"av1C", &alpha.config), true),
                    properties.add(full_box(b"auxC", 0, 0, ALPHA_URN), true),
                ],
            ));
            locations.push((ALPHA_ID, data_start + image.data.len(), alpha.data.len()));
            infos.push((ALPHA_ID, b"av01", false));
        }
        let ipco = plain_box(b"ipco", &properties.boxes.concat());
        body.extend([
            iloc(&locations),
            iinf(&infos),
            plain_box(b"iprp", &[ipco, ipma(&items)].concat()),
        ]);
        if alpha.is_some() {
            let auxl = [
                ALPHA_ID.to_be_bytes(),
                1u16.to_be_bytes(),
                BASE_ID.to_be_bytes(),
            ];
            body.push(full_box(b"iref", 0, 0, &plain_box(b"auxl", &auxl.concat())));
        }
        full_box(b"meta", 0, 0, &body.concat())
    };
    let data = [&image.data[..], alpha.map_or(&[][..], |alpha| &alpha.data)].concat();
    // Item offsets are absolute, and the meta box's size doesn't depend on them.
    let data_start = ftyp.len() + meta(0).len() + 8;
    [ftyp, meta(data_start), plain_box(b"mdat", &data)].concat()
}

/// `base` and `gain_map` as one AVIF, `base` being in `color`.
pub fn write(
    base: &Av1Image,
    gain_map: &Av1Image,
    metadata: &GainMapMetadata,
    color: &BaseColor,
) -> Vec<u8> {
    let tone_map = [&[0][..], &metadata.to_iso()].concat();
    let ftyp = plain_box(b"ftyp", &[BRANDS, b"tmap"].concat());
    let file = File {
        base,
        gain_map,
        tone_map: &tone_map,
        color,
    };
    // Item offsets are absolute, and the meta box's size doesn't depend on them.
    let meta_len = file.meta(0).len();
    let data_start = ftyp.len() + meta_len + 8;
    let meta = file.meta(data_start);
    let mdat = plain_box(
        b"mdat",
        &[&tone_map[..], &gain_map.data, &base.data].concat(),
    );
    [ftyp, meta, mdat].concat()
}

struct File<'a> {
    base: &'a Av1Image,
    gain_map: &'a Av1Image,
    /// The `tmap` item's payload.
    tone_map: &'a [u8],
    color: &'a BaseColor,
}

impl File<'_> {
    /// The meta box, for item data starting at file offset `data_start` in
    /// this order: tone map, gain map, base image.
    fn meta(&self, data_start: usize) -> Vec<u8> {
        let tone_map_at = data_start;
        let gain_map_at = tone_map_at + self.tone_map.len();
        let base_at = gain_map_at + self.gain_map.data.len();

        // The tone-mapped image derives from the base, then the gain map.
        let mut dimg = TONE_MAP_ID.to_be_bytes().to_vec();
        let derived_from = [BASE_ID, GAIN_MAP_ID];
        dimg.extend((derived_from.len() as u16).to_be_bytes());
        for id in derived_from {
            dimg.extend(id.to_be_bytes());
        }

        let mut altr = ALTERNATIVES_GROUP_ID.to_be_bytes().to_vec();
        let preferred_first = [TONE_MAP_ID, BASE_ID];
        altr.extend((preferred_first.len() as u32).to_be_bytes());
        for id in preferred_first {
            altr.extend(u32::from(id).to_be_bytes());
        }

        let body = [
            hdlr(),
            full_box(b"pitm", 0, 0, &BASE_ID.to_be_bytes()),
            iloc(&[
                (BASE_ID, base_at, self.base.data.len()),
                (TONE_MAP_ID, tone_map_at, self.tone_map.len()),
                (GAIN_MAP_ID, gain_map_at, self.gain_map.data.len()),
            ]),
            iinf(&[
                (BASE_ID, b"av01", false),
                (TONE_MAP_ID, b"tmap", false),
                (GAIN_MAP_ID, b"av01", true),
            ]),
            full_box(b"iref", 0, 0, &plain_box(b"dimg", &dimg)),
            self.iprp(),
            plain_box(b"grpl", &full_box(b"altr", 0, 0, &altr)),
        ];
        full_box(b"meta", 0, 0, &body.concat())
    }

    fn iprp(&self) -> Vec<u8> {
        let mut properties = Properties::default();
        let colors = |properties: &mut Properties| match self.color {
            BaseColor::Icc(icc) => vec![
                properties.add(plain_box(b"colr", &[&b"prof"[..], icc].concat()), false),
                properties.add(nclx(MATRIX_ONLY), false),
            ],
            &BaseColor::Cicp {
                primaries,
                transfer,
            } => vec![properties.add(nclx((primaries, transfer, SDR_MATRIX)), false)],
        };

        let mut base = vec![
            properties.add(ispe(self.base), false),
            properties.add(pixi(self.base), false),
            properties.add(plain_box(b"av1C", &self.base.config), true),
        ];
        base.extend(colors(&mut properties));

        let mut tone_map = vec![properties.add(ispe(self.base), false)];
        tone_map.extend(colors(&mut properties));

        let gain_map = vec![
            properties.add(ispe(self.gain_map), false),
            properties.add(pixi(self.gain_map), false),
            properties.add(plain_box(b"av1C", &self.gain_map.config), true),
            properties.add(nclx(MATRIX_ONLY), false),
        ];

        let ipma = ipma(&[
            (BASE_ID, base),
            (TONE_MAP_ID, tone_map),
            (GAIN_MAP_ID, gain_map),
        ]);
        let ipco = plain_box(b"ipco", &properties.boxes.concat());
        plain_box(b"iprp", &[ipco, ipma].concat())
    }
}

fn hdlr() -> Vec<u8> {
    full_box(b"hdlr", 0, 0, &[&[0; 4][..], b"pict", &[0; 13]].concat())
}

/// Item locations: ID, absolute offset, length.
fn iloc(items: &[(u16, usize, usize)]) -> Vec<u8> {
    let mut body = vec![0x44, 0x00];
    body.extend((items.len() as u16).to_be_bytes());
    for &(id, at, len) in items {
        body.extend(id.to_be_bytes());
        // Data reference index 0 (this file), and one extent.
        body.extend([0, 0, 0, 1]);
        body.extend(u32_len(at).to_be_bytes());
        body.extend(u32_len(len).to_be_bytes());
    }
    full_box(b"iloc", 0, 0, &body)
}

/// Item infos: ID, type, whether hidden.
fn iinf(items: &[(u16, &[u8; 4], bool)]) -> Vec<u8> {
    let mut body = (items.len() as u16).to_be_bytes().to_vec();
    for &(id, kind, hidden) in items {
        let mut infe = id.to_be_bytes().to_vec();
        infe.extend([0, 0]);
        infe.extend(kind);
        infe.push(0);
        body.extend(full_box(b"infe", 2, u32::from(hidden), &infe));
    }
    full_box(b"iinf", 0, 0, &body)
}

/// Item IDs with their property association bytes.
fn ipma(items: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut body = (items.len() as u32).to_be_bytes().to_vec();
    for (id, associations) in items {
        body.extend(id.to_be_bytes());
        body.push(associations.len() as u8);
        body.extend(associations);
    }
    full_box(b"ipma", 0, 0, &body)
}

/// Item properties, each stored once however many items share it.
#[derive(Default)]
struct Properties {
    boxes: Vec<Vec<u8>>,
}

impl Properties {
    /// The `ipma` association byte: the 1-based index, and the essential bit.
    fn add(&mut self, property: Vec<u8>, essential: bool) -> u8 {
        let index = match self.boxes.iter().position(|known| *known == property) {
            Some(index) => index,
            None => {
                self.boxes.push(property);
                self.boxes.len() - 1
            }
        };
        // Seven bits; a few properties per file.
        (index + 1) as u8 | if essential { 0x80 } else { 0 }
    }
}

fn ispe(image: &Av1Image) -> Vec<u8> {
    let size = [image.width.to_be_bytes(), image.height.to_be_bytes()].concat();
    full_box(b"ispe", 0, 0, &size)
}

fn pixi(image: &Av1Image) -> Vec<u8> {
    let channels: u8 = if image.monochrome { 1 } else { 3 };
    let mut body = vec![channels];
    body.extend(std::iter::repeat_n(BIT_DEPTH, usize::from(channels)));
    full_box(b"pixi", 0, 0, &body)
}

/// A full-range `colr` box.
fn nclx((primaries, transfer, matrix): Cicp) -> Vec<u8> {
    let mut body = b"nclx".to_vec();
    for code in [primaries, transfer, matrix] {
        body.extend(code.to_be_bytes());
    }
    body.push(0x80);
    plain_box(b"colr", &body)
}

/// A box without the version and flags of a full box.
fn plain_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    [&u32_len(8 + body.len()).to_be_bytes()[..], kind, body].concat()
}

fn full_box(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let header = (u32::from(version) << 24 | flags).to_be_bytes();
    plain_box(kind, &[&header[..], body].concat())
}

fn u32_len(len: usize) -> u32 {
    u32::try_from(len).expect("previews are far smaller than 4 GiB")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gain_map::{ChannelMapping, Fraction, UFraction};

    fn image(data: &[u8], monochrome: bool) -> Av1Image {
        Av1Image {
            data: data.to_vec(),
            config: vec![0x81, 0, 0, 0],
            width: 4,
            height: 2,
            monochrome,
        }
    }

    fn metadata() -> GainMapMetadata {
        let fraction = Fraction { n: 1, d: 2 };
        let channel = ChannelMapping {
            gain_map_min: fraction,
            gain_map_max: fraction,
            gamma: UFraction { n: 1, d: 1 },
            base_offset: fraction,
            alternate_offset: fraction,
        };
        GainMapMetadata {
            use_base_color_space: true,
            base_headroom: UFraction { n: 0, d: 1 },
            alternate_headroom: UFraction { n: 2, d: 1 },
            channels: [channel; 3],
        }
    }

    /// The top-level boxes' types and bodies.
    fn boxes(mut data: &[u8]) -> Vec<(&[u8], &[u8])> {
        let mut found = Vec::new();
        while !data.is_empty() {
            let len = u32::from_be_bytes(data[..4].try_into().unwrap()) as usize;
            found.push((&data[4..8], &data[8..len]));
            data = &data[len..];
        }
        found
    }

    fn find<'a>(data: &'a [u8], path: &[&[u8]]) -> &'a [u8] {
        let (kind, rest) = path.split_first().unwrap();
        let (_, body) = boxes(data)
            .into_iter()
            .find(|(k, _)| k == kind)
            .unwrap_or_else(|| panic!("no {}", String::from_utf8_lossy(kind)));
        if rest.is_empty() {
            body
        } else {
            // `meta` is a full box; its children follow the version and flags.
            let skip = if *kind == b"meta" { 4 } else { 0 };
            find(&body[skip..], rest)
        }
    }

    #[test]
    fn writes_a_tone_mapped_file() {
        let (base, gain_map) = (image(b"BASE-OBUS", false), image(b"MAP", true));
        let file = write(
            &base,
            &gain_map,
            &metadata(),
            &BaseColor::Icc(b"ICC".to_vec()),
        );

        let kinds: Vec<_> = boxes(&file).into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(kinds, [&b"ftyp"[..], b"meta", b"mdat"]);
        assert!(find(&file, &[b"ftyp"]).ends_with(b"tmap"));

        assert_eq!(extent(&file, 0), b"BASE-OBUS");
        assert_eq!(extent(&file, 1), [&[0][..], &metadata().to_iso()].concat());
        assert_eq!(extent(&file, 2), b"MAP");

        let altr = find(&file, &[b"meta", b"grpl", b"altr"]);
        assert_eq!(altr[8..], [0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 1]);
    }

    #[test]
    fn writes_cicp_base_colors() {
        let color = BaseColor::Cicp {
            primaries: 9,
            transfer: 1,
        };
        let file = write(&image(b"B", false), &image(b"M", true), &metadata(), &color);
        let ipco = find(&file, &[b"meta", b"iprp", b"ipco"]);
        assert!(contains(ipco, &nclx((9, 1, SDR_MATRIX))));
        assert!(!contains(ipco, b"prof"));
    }

    #[test]
    fn writes_a_pq_file() {
        let file = write_pq(&image(b"PQ-OBUS", false), None);

        let kinds: Vec<_> = boxes(&file).into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(kinds, [&b"ftyp"[..], b"meta", b"mdat"]);
        assert_eq!(extent(&file, 0), b"PQ-OBUS");

        let ipco = find(&file, &[b"meta", b"iprp", b"ipco"]);
        assert!(contains(ipco, &nclx(BT2100_PQ)));
        assert!(!contains(ipco, ALPHA_URN));
    }

    #[test]
    fn writes_a_pq_file_with_alpha() {
        let file = write_pq(&image(b"PQ-OBUS", false), Some(&image(b"ALPHA", true)));

        assert_eq!(extent(&file, 0), b"PQ-OBUS");
        assert_eq!(extent(&file, 1), b"ALPHA");
        let ipco = find(&file, &[b"meta", b"iprp", b"ipco"]);
        assert!(contains(ipco, ALPHA_URN));
        let auxl = find(&file, &[b"meta", b"iref"]);
        // After the version and flags: from the alpha item, one reference, to the image.
        assert_eq!(
            auxl[4..],
            [&[0, 0, 0, 14][..], b"auxl", &[0, 2, 0, 1, 0, 1]].concat()
        );
    }

    /// The data of the `i`-th item in `file`'s `iloc`.
    fn extent(file: &[u8], i: usize) -> &[u8] {
        let entry = &find(file, &[b"meta", b"iloc"])[8 + i * 14..];
        let at = u32::from_be_bytes(entry[6..10].try_into().unwrap()) as usize;
        let len = u32::from_be_bytes(entry[10..14].try_into().unwrap()) as usize;
        &file[at..at + len]
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn shares_identical_properties() {
        let mut properties = Properties::default();
        assert_eq!(properties.add(nclx(BT2100_PQ), false), 1);
        assert_eq!(properties.add(nclx(MATRIX_ONLY), true), 0x82);
        assert_eq!(properties.add(nclx(BT2100_PQ), false), 1);
        assert_eq!(properties.boxes.len(), 2);
    }
}
