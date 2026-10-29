//! The items of a HEIF file held in memory: their types, data, properties
//! and references, from the `meta` box.

use super::probe::{BoxType, be_u16, be_u32, be_u64, boxes, parse_ipma, parse_pitm};

pub struct Items<'a> {
    file: &'a [u8],
    pub primary: u32,
    types: Vec<(u32, BoxType)>,
    locations: Vec<Location>,
    properties: Vec<(BoxType, &'a [u8])>,
    /// Each item's 1-based property indices, in order.
    associations: Vec<(u32, Vec<u16>)>,
    references: Vec<Reference>,
    /// Data of items stored in the `meta` box itself.
    idat: &'a [u8],
}

struct Location {
    id: u32,
    /// Whether offsets are into `idat` rather than the file.
    in_idat: bool,
    /// Offset and length of each piece; a length of 0 runs to the end.
    extents: Vec<(u64, u64)>,
}

struct Reference {
    kind: BoxType,
    from: u32,
    to: Vec<u32>,
}

impl<'a> Items<'a> {
    /// `None` without a `meta` box or a primary item.
    pub fn parse(file: &'a [u8]) -> Option<Self> {
        let (_, meta) = boxes(file).find(|(kind, _)| kind == b"meta")?;
        let mut items = Self {
            file,
            primary: 0,
            types: Vec::new(),
            locations: Vec::new(),
            properties: Vec::new(),
            associations: Vec::new(),
            references: Vec::new(),
            idat: &[],
        };
        let mut primary = None;
        // `meta` is a full box: version and flags come before the children.
        for (kind, body) in boxes(meta.get(4..)?) {
            match &kind {
                b"pitm" => primary = parse_pitm(body),
                b"iinf" => items.types = parse_iinf(body)?,
                b"iloc" => items.locations = parse_iloc(body)?,
                b"iref" => items.references = parse_iref(body)?,
                b"idat" => items.idat = body,
                b"iprp" => {
                    for (kind, body) in boxes(body) {
                        match &kind {
                            b"ipco" => items.properties = boxes(body).collect(),
                            b"ipma" => items.associations = parse_ipma(body)?,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        items.primary = primary?;
        Some(items)
    }

    pub fn item_type(&self, id: u32) -> Option<BoxType> {
        self.types
            .iter()
            .find_map(|(item, kind)| (*item == id).then_some(*kind))
    }

    /// The first item of type `kind`.
    pub fn find_type(&self, kind: &BoxType) -> Option<u32> {
        self.types
            .iter()
            .find_map(|(item, item_kind)| (item_kind == kind).then_some(*item))
    }

    /// The item's data, its pieces joined.
    pub fn data(&self, id: u32) -> Option<Vec<u8>> {
        let location = self.locations.iter().find(|location| location.id == id)?;
        let source = if location.in_idat {
            self.idat
        } else {
            self.file
        };
        let mut data = Vec::new();
        for &(offset, len) in &location.extents {
            let start = usize::try_from(offset).ok()?;
            let end = match len {
                0 => source.len(),
                len => start.checked_add(usize::try_from(len).ok()?)?,
            };
            data.extend_from_slice(source.get(start..end)?);
        }
        Some(data)
    }

    /// The item's properties, in the order they apply.
    pub fn properties(&self, id: u32) -> impl Iterator<Item = (BoxType, &'a [u8])> + '_ {
        self.associations
            .iter()
            .filter(move |(item, _)| *item == id)
            .flat_map(|(_, indices)| indices)
            // 0 means none.
            .filter_map(|&index| self.properties.get(usize::from(index).checked_sub(1)?))
            .copied()
    }

    /// The body of the item's first property of type `kind`.
    pub fn property(&self, id: u32, kind: &BoxType) -> Option<&'a [u8]> {
        self.properties(id)
            .find_map(|(property, body)| (&property == kind).then_some(body))
    }

    /// The items `from` refers to by a `kind` reference.
    pub fn references_from(&self, kind: &BoxType, from: u32) -> &[u32] {
        self.references
            .iter()
            .find(|reference| &reference.kind == kind && reference.from == from)
            .map_or(&[], |reference| &reference.to)
    }

    /// The items that refer to `to` by a `kind` reference.
    pub fn references_to(&self, kind: BoxType, to: u32) -> impl Iterator<Item = u32> + '_ {
        self.references
            .iter()
            .filter(move |reference| reference.kind == kind && reference.to.contains(&to))
            .map(|reference| reference.from)
    }
}

/// Each item's ID and type.
fn parse_iinf(body: &[u8]) -> Option<Vec<(u32, BoxType)>> {
    let entries_at = if *body.first()? == 0 { 6 } else { 8 };
    Some(
        boxes(body.get(entries_at..)?)
            .filter(|(kind, _)| kind == b"infe")
            .filter_map(|(_, infe)| parse_infe(infe))
            .collect(),
    )
}

/// An item's ID and type; versions before 2 have no type.
fn parse_infe(infe: &[u8]) -> Option<(u32, BoxType)> {
    let (id, type_at) = match infe.first()? {
        2 => (be_u16(infe, 4)?.into(), 8),
        3 => (be_u32(infe, 4)?, 10),
        _ => return None,
    };
    Some((id, infe.get(type_at..type_at + 4)?.try_into().ok()?))
}

fn parse_iloc(body: &[u8]) -> Option<Vec<Location>> {
    let mut data = Reader { data: body, at: 0 };
    let version = data.uint(1)?;
    // The flags.
    data.at += 3;
    let sizes = data.uint(2)?;
    let [offset_size, len_size, base_offset_size, index_size] =
        [12, 8, 4, 0].map(|shift| ((sizes >> shift) & 0xF) as usize);
    let index_size = if version == 0 { 0 } else { index_size };
    let count = data.uint(if version < 2 { 2 } else { 4 })?;
    let mut locations = Vec::new();
    for _ in 0..count {
        let id = data.uint(if version < 2 { 2 } else { 4 })? as u32;
        let method = if version == 0 { 0 } else { data.uint(2)? & 0xF };
        let _data_reference = data.uint(2)?;
        let base = data.uint(base_offset_size)?;
        let extent_count = data.uint(2)?;
        let mut extents = Vec::new();
        for _ in 0..extent_count {
            data.uint(index_size)?;
            let offset = base.checked_add(data.uint(offset_size)?)?;
            extents.push((offset, data.uint(len_size)?));
        }
        // Method 2, by reference to another item's data, is unsupported.
        if method < 2 {
            locations.push(Location {
                id,
                in_idat: method == 1,
                extents,
            });
        }
    }
    Some(locations)
}

fn parse_iref(body: &[u8]) -> Option<Vec<Reference>> {
    let id_len = if *body.first()? == 0 { 2 } else { 4 };
    let mut references = Vec::new();
    for (kind, body) in boxes(body.get(4..)?) {
        let mut data = Reader { data: body, at: 0 };
        let from = data.uint(id_len)? as u32;
        let count = data.uint(2)?;
        let to = (0..count)
            .map(|_| data.uint(id_len).map(|id| id as u32))
            .collect::<Option<_>>()?;
        references.push(Reference { kind, from, to });
    }
    Some(references)
}

/// Big-endian unsigned integers of varying sizes.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn uint(&mut self, len: usize) -> Option<u64> {
        let value = match len {
            0 => 0,
            1 => (*self.data.get(self.at)?).into(),
            2 => be_u16(self.data, self.at)?.into(),
            4 => be_u32(self.data, self.at)?.into(),
            8 => be_u64(self.data, self.at)?,
            _ => return None,
        };
        self.at += len;
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        av1::Av1Image,
        gain_map::{ChannelMapping, Fraction, GainMapMetadata, UFraction},
        heif::avif::{self, ColorSpace},
    };

    fn image(data: &[u8]) -> Av1Image {
        Av1Image {
            data: data.to_vec(),
            config: vec![0x81, 0, 0, 0],
            width: 4,
            height: 2,
            monochrome: false,
        }
    }

    #[test]
    fn reads_what_the_writer_wrote() {
        let fraction = Fraction { n: 1, d: 2 };
        let channel = ChannelMapping {
            gain_map_min: fraction,
            gain_map_max: fraction,
            gamma: UFraction { n: 1, d: 1 },
            base_offset: fraction,
            alternate_offset: fraction,
        };
        let metadata = GainMapMetadata {
            use_base_color_space: true,
            base_headroom: UFraction { n: 0, d: 1 },
            alternate_headroom: UFraction { n: 2, d: 1 },
            channels: [channel; 3],
        };
        let file = avif::write(
            &image(b"BASE"),
            &image(b"MAP"),
            &metadata,
            &ColorSpace::Icc(b"ICC".to_vec()),
        );

        let items = Items::parse(&file).unwrap();
        assert_eq!(items.item_type(items.primary), Some(*b"av01"));
        assert_eq!(items.data(items.primary).unwrap(), b"BASE");
        let tone_map = items.find_type(b"tmap").unwrap();
        let [base, gain_map] = items.references_from(b"dimg", tone_map) else {
            panic!("no dimg");
        };
        assert_eq!(*base, items.primary);
        assert_eq!(items.data(*gain_map).unwrap(), b"MAP");
        assert_eq!(items.data(tone_map).unwrap()[1..], metadata.to_iso());
        assert_eq!(
            items.property(items.primary, b"av1C"),
            Some(&[0x81, 0, 0, 0][..])
        );
        assert!(items.property(*gain_map, b"colr").is_some());
    }

    #[test]
    fn reads_alpha_references() {
        let file = avif::write_pq(&image(b"PQ"), Some(&image(b"ALPHA")));
        let items = Items::parse(&file).unwrap();
        let alpha: Vec<_> = items.references_to(*b"auxl", items.primary).collect();
        assert_eq!(alpha.len(), 1);
        assert_eq!(items.data(alpha[0]).unwrap(), b"ALPHA");
    }

    #[test]
    fn reads_idat_and_wide_iloc_fields() {
        // Version 1: 4-byte offsets and lengths, 8-byte base offsets, no index.
        let mut iloc = vec![1, 0, 0, 0, 0x44, 0x80, 0, 1];
        iloc.extend([0, 7, 0, 1, 0, 0]);
        iloc.extend(2u64.to_be_bytes());
        iloc.extend([0, 1]);
        iloc.extend(1u32.to_be_bytes());
        iloc.extend(3u32.to_be_bytes());
        let mut locations = parse_iloc(&iloc).unwrap();
        let location = locations.pop().unwrap();
        assert_eq!((location.id, location.in_idat), (7, true));
        assert_eq!(location.extents, [(3, 3)]);
    }

    #[test]
    fn needs_a_meta_box() {
        assert!(Items::parse(b"\0\0\0\x08free").is_none());
    }
}
