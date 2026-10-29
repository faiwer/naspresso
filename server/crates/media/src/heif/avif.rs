//! Gain-map AVIF files (ISO/IEC 23008-12 tone-map derived items): the SDR base
//! image as the primary item, so older readers still show it, and a `tmap`
//! item that HDR-capable readers prefer, deriving the HDR rendition from the
//! base and a hidden gain map.

use crate::{
    av1::{Av1Image, BIT_DEPTH},
    gain_map::GainMapMetadata,
};

const BASE_ID: u16 = 1;
const TONE_MAP_ID: u16 = 2;
const GAIN_MAP_ID: u16 = 3;
/// Must differ from every item ID.
const ALTERNATIVES_GROUP_ID: u32 = 4;

/// CICP codes for `colr` boxes: primaries, transfer, matrix.
type Cicp = (u16, u16, u16);
/// sRGB, BT.601 matrix: what `av1::encode` codes in the bitstream.
const SRGB: Cicp = (1, 13, 6);
/// The ICC profile has the primaries and transfer; only the matrix is here.
const MATRIX_ONLY: Cicp = (2, 2, 6);

/// `base` and `gain_map` as one AVIF. `icc` is the base image's color profile,
/// which the HDR rendition shares; sRGB without one.
pub fn write(
    base: &Av1Image,
    gain_map: &Av1Image,
    metadata: &GainMapMetadata,
    icc: Option<&[u8]>,
) -> Vec<u8> {
    let tone_map = [&[0][..], &metadata.to_iso()].concat();
    // Major brand, minor version 0, compatible brands.
    let ftyp = plain_box(b"ftyp", b"avif\0\0\0\0avifmif1miafMA1Atmap");
    let file = File {
        base,
        gain_map,
        tone_map: &tone_map,
        icc,
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
    icc: Option<&'a [u8]>,
}

impl File<'_> {
    /// The meta box, for item data starting at file offset `data_start` in
    /// this order: tone map, gain map, base image.
    fn meta(&self, data_start: usize) -> Vec<u8> {
        let tone_map_at = data_start;
        let gain_map_at = tone_map_at + self.tone_map.len();
        let base_at = gain_map_at + self.gain_map.data.len();

        let mut iloc = vec![0x44, 0x00];
        iloc.extend(3u16.to_be_bytes());
        for (id, at, len) in [
            (BASE_ID, base_at, self.base.data.len()),
            (TONE_MAP_ID, tone_map_at, self.tone_map.len()),
            (GAIN_MAP_ID, gain_map_at, self.gain_map.data.len()),
        ] {
            iloc.extend(id.to_be_bytes());
            // Data reference index 0 (this file), and one extent.
            iloc.extend([0, 0, 0, 1]);
            iloc.extend(u32_len(at).to_be_bytes());
            iloc.extend(u32_len(len).to_be_bytes());
        }

        let mut iinf = 3u16.to_be_bytes().to_vec();
        for (id, kind, hidden) in [
            (BASE_ID, b"av01", false),
            (TONE_MAP_ID, b"tmap", false),
            (GAIN_MAP_ID, b"av01", true),
        ] {
            let mut infe = id.to_be_bytes().to_vec();
            infe.extend([0, 0]);
            infe.extend(kind);
            infe.push(0);
            iinf.extend(full_box(b"infe", 2, u32::from(hidden), &infe));
        }

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
            full_box(b"hdlr", 0, 0, &[&[0; 4][..], b"pict", &[0; 13]].concat()),
            full_box(b"pitm", 0, 0, &BASE_ID.to_be_bytes()),
            full_box(b"iloc", 0, 0, &iloc),
            full_box(b"iinf", 0, 0, &iinf),
            full_box(b"iref", 0, 0, &plain_box(b"dimg", &dimg)),
            self.iprp(),
            plain_box(b"grpl", &full_box(b"altr", 0, 0, &altr)),
        ];
        full_box(b"meta", 0, 0, &body.concat())
    }

    fn iprp(&self) -> Vec<u8> {
        let mut properties = Properties::default();
        let colors = |properties: &mut Properties| match self.icc {
            Some(icc) => vec![
                properties.add(plain_box(b"colr", &[b"prof", icc].concat()), false),
                properties.add(nclx(MATRIX_ONLY), false),
            ],
            None => vec![properties.add(nclx(SRGB), false)],
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

        let mut ipma = 3u32.to_be_bytes().to_vec();
        for (id, associations) in [
            (BASE_ID, base),
            (TONE_MAP_ID, tone_map),
            (GAIN_MAP_ID, gain_map),
        ] {
            ipma.extend(id.to_be_bytes());
            ipma.push(associations.len() as u8);
            ipma.extend(associations);
        }

        let ipco = plain_box(b"ipco", &properties.boxes.concat());
        plain_box(b"iprp", &[ipco, full_box(b"ipma", 0, 0, &ipma)].concat())
    }
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
        let file = write(&base, &gain_map, &metadata(), Some(b"ICC"));

        let kinds: Vec<_> = boxes(&file).into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(kinds, [&b"ftyp"[..], b"meta", b"mdat"]);
        assert!(find(&file, &[b"ftyp"]).ends_with(b"tmap"));

        let iloc = find(&file, &[b"meta", b"iloc"]);
        let extent = |i: usize| {
            let entry = &iloc[8 + i * 14..];
            let at = u32::from_be_bytes(entry[6..10].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(entry[10..14].try_into().unwrap()) as usize;
            &file[at..at + len]
        };
        assert_eq!(extent(0), b"BASE-OBUS");
        assert_eq!(extent(1), [&[0][..], &metadata().to_iso()].concat());
        assert_eq!(extent(2), b"MAP");

        let altr = find(&file, &[b"meta", b"grpl", b"altr"]);
        assert_eq!(altr[8..], [0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 1]);
    }

    #[test]
    fn shares_identical_properties() {
        let mut properties = Properties::default();
        assert_eq!(properties.add(nclx(SRGB), false), 1);
        assert_eq!(properties.add(nclx(MATRIX_ONLY), true), 0x82);
        assert_eq!(properties.add(nclx(SRGB), false), 1);
        assert_eq!(properties.boxes.len(), 2);
    }
}
