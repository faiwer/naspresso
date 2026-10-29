//! AVIF images held in memory, decoded upright: the primary image with its
//! alpha, or a gain-map file's base image and gain map.

use fast_image_resize::{PixelType, images::Image};

use super::{
    avif::{ALPHA_URN, ColorSpace, is_rgb_profile},
    items::Items,
};
use crate::{
    av1::decode::{Cicp, DecodeError, Frame, Samples, decode as decode_av1},
    gain_map::GainMapMetadata,
};

/// HEVC's alpha auxiliary type, which some AVIF writers use too.
const HEVC_ALPHA_URN: &[u8] = b"urn:mpeg:hevc:2015:auxid:1\0";

pub struct Decoded {
    pub image: Image<'static>,
    pub color: ColorSpace,
}

/// The primary image of the AVIF `file` as 8-bit RGB or, if `wide`, 16-bit:
/// `U8x3` or `U16x3`, `U8x4` or `U16x4` with alpha.
pub fn decode(file: &[u8], wide: bool) -> Result<Decoded, DecodeError> {
    let items = parse(file)?;
    let primary = decode_item(&items, items.primary)?;
    let mut samples = primary.rgb()?;
    if let Some(alpha) = decode_alpha(&items, &primary)? {
        samples = with_alpha(samples, &alpha);
    }
    let samples = upright(&items, items.primary, samples);
    Ok(Decoded {
        image: to_image(&samples, wide)?,
        color: primary.color,
    })
}

/// A base image and its gain map, as the gain map's metadata says to combine
/// them.
pub struct GainMapped {
    pub metadata: GainMapMetadata,
    /// 8-bit RGB.
    pub base: Image<'static>,
    pub color: ColorSpace,
    /// 8-bit RGB or grayscale.
    pub gain_map: Image<'static>,
}

/// The base image and gain map of the AVIF `file`'s tone-map item.
pub fn decode_gain_mapped(file: &[u8]) -> Result<GainMapped, DecodeError> {
    let items = parse(file)?;
    let tone_map = items
        .find_type(b"tmap")
        .ok_or_else(|| DecodeError::new("AVIF without a tone-map item"))?;
    let &[base, gain_map] = items.references_from(b"dimg", tone_map) else {
        return Err(DecodeError::new("tone-map item without its two inputs"));
    };
    let metadata = items
        .data(tone_map)
        .and_then(|payload| match payload.split_first() {
            Some((0, iso)) => GainMapMetadata::from_iso(iso),
            _ => None,
        })
        .ok_or_else(|| DecodeError::new("unknown gain map metadata"))?;

    let base_image = decode_item(&items, base)?;
    let map_image = decode_item(&items, gain_map)?;
    let map = if map_image.monochrome() {
        map_image.luma()?
    } else {
        map_image.rgb()?
    };
    // The gain map follows the base image's orientation.
    let base_samples = upright(&items, base, base_image.rgb()?);
    let map = upright(&items, base, map);
    Ok(GainMapped {
        metadata,
        base: to_image(&base_samples, false)?,
        color: base_image.color,
        gain_map: to_image(&map, false)?,
    })
}

fn parse(file: &[u8]) -> Result<Items<'_>, DecodeError> {
    Items::parse(file).ok_or_else(|| DecodeError::new("AVIF without a primary item"))
}

/// An item's image: one coded frame, or a grid of equally sized ones.
struct Coded {
    tiles: Vec<Frame>,
    grid: Option<Grid>,
    /// The codes to read the frames by.
    cicp: Cicp,
    color: ColorSpace,
}

impl Coded {
    fn width(&self) -> u32 {
        self.grid
            .as_ref()
            .map_or_else(|| self.tiles[0].width(), |grid| grid.width)
    }

    fn height(&self) -> u32 {
        self.grid
            .as_ref()
            .map_or_else(|| self.tiles[0].height(), |grid| grid.height)
    }

    fn monochrome(&self) -> bool {
        self.tiles[0].monochrome()
    }

    fn rgb(&self) -> Result<Samples, DecodeError> {
        self.stitch(|tile| tile.rgb(self.cicp))
    }

    fn luma(&self) -> Result<Samples, DecodeError> {
        self.stitch(|tile| Ok(tile.luma(self.cicp.full_range)))
    }

    /// The tiles' samples, laid out row by row and cropped to the grid's size.
    fn stitch(
        &self,
        samples: impl Fn(&Frame) -> Result<Samples, DecodeError>,
    ) -> Result<Samples, DecodeError> {
        let Some(grid) = &self.grid else {
            return samples(&self.tiles[0]);
        };
        let mut canvas: Option<Samples> = None;
        for (i, tile) in self.tiles.iter().enumerate() {
            let tile = samples(tile)?;
            let canvas = canvas.get_or_insert_with(|| Samples {
                width: grid.width,
                height: grid.height,
                channels: tile.channels,
                max: tile.max,
                values: vec![0; grid.width as usize * grid.height as usize * tile.channels],
            });
            if (tile.channels, tile.max) != (canvas.channels, canvas.max) {
                return Err(DecodeError::new("AVIF grid tiles of different formats"));
            }
            let left = (i % grid.columns) as u32 * tile.width;
            let top = (i / grid.columns) as u32 * tile.height;
            place(canvas, &tile, left, top);
        }
        canvas.ok_or_else(|| DecodeError::new("AVIF grid without tiles"))
    }
}

/// A `grid` item's layout.
#[derive(Debug, PartialEq, Eq)]
struct Grid {
    rows: usize,
    columns: usize,
    width: u32,
    height: u32,
}

fn parse_grid(data: &[u8]) -> Option<Grid> {
    let (&[_version, flags, rows, columns], sizes) = data.split_first_chunk()?;
    let len = if flags & 1 == 0 { 2 } else { 4 };
    let size = |at: usize| {
        let bytes = sizes.get(at * len..(at + 1) * len)?;
        Some(bytes.iter().fold(0, |n, &b| n << 8 | u32::from(b)))
    };
    Some(Grid {
        rows: usize::from(rows) + 1,
        columns: usize::from(columns) + 1,
        width: size(0)?,
        height: size(1)?,
    })
}

/// Copies `tile` into `canvas` at `left`, `top`, dropping what falls outside.
fn place(canvas: &mut Samples, tile: &Samples, left: u32, top: u32) {
    let channels = canvas.channels;
    let visible = canvas.width.saturating_sub(left).min(tile.width) as usize * channels;
    let rows = canvas.height.saturating_sub(top).min(tile.height) as usize;
    let tile_rows = tile.values.chunks_exact(tile.width as usize * channels);
    for (y, row) in tile_rows.take(rows).enumerate() {
        let at = ((top as usize + y) * canvas.width as usize + left as usize) * channels;
        canvas.values[at..at + visible].copy_from_slice(&row[..visible]);
    }
}

/// The item's frames and how to read them.
fn decode_item(items: &Items, id: u32) -> Result<Coded, DecodeError> {
    let (tile_ids, grid) = match items.item_type(id).as_ref() {
        Some(b"av01") => (vec![id], None),
        Some(b"grid") => {
            let grid = items
                .data(id)
                .as_deref()
                .and_then(parse_grid)
                .ok_or_else(|| DecodeError::new("unreadable AVIF grid"))?;
            let tiles = items.references_from(b"dimg", id);
            if tiles.len() != grid.rows * grid.columns {
                return Err(DecodeError::new("AVIF grid with missing tiles"));
            }
            (tiles.to_vec(), Some(grid))
        }
        _ => return Err(DecodeError::new("AVIF item isn't AV1")),
    };
    let tiles = tile_ids
        .iter()
        .map(|&tile| {
            if items.item_type(tile).as_ref() != Some(b"av01") {
                return Err(DecodeError::new("AVIF grid tile isn't AV1"));
            }
            let data = items
                .data(tile)
                .ok_or_else(|| DecodeError::new("AVIF item without data"))?;
            decode_av1(&data)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(grid) = &grid {
        let (width, height) = (tiles[0].width(), tiles[0].height());
        let covered = width as usize * grid.columns >= grid.width as usize
            && height as usize * grid.rows >= grid.height as usize;
        if !covered
            || tiles
                .iter()
                .any(|tile| (tile.width(), tile.height()) != (width, height))
        {
            return Err(DecodeError::new("AVIF grid tiles don't fit it"));
        }
    }

    // A grid's tiles may carry its color properties instead.
    let (nclx, icc) = color_properties(items, id);
    let (tile_nclx, tile_icc) = color_properties(items, tile_ids[0]);
    // The container's codes override the bitstream's.
    let cicp = nclx.or(tile_nclx).unwrap_or_else(|| tiles[0].cicp());
    let color = match icc.or(tile_icc) {
        Some(icc) if is_rgb_profile(&icc) => ColorSpace::Icc(icc),
        _ => ColorSpace::Cicp {
            primaries: cicp.primaries,
            transfer: cicp.transfer,
        },
    };
    Ok(Coded {
        tiles,
        grid,
        cicp,
        color,
    })
}

/// The item's first `nclx` codes and ICC profile.
fn color_properties(items: &Items, id: u32) -> (Option<Cicp>, Option<Vec<u8>>) {
    let (mut nclx, mut icc) = (None, None);
    for (kind, body) in items.properties(id) {
        if &kind != b"colr" {
            continue;
        }
        match body.get(..4) {
            Some(b"nclx") => nclx = nclx.or_else(|| parse_nclx(body)),
            Some(b"prof" | b"rICC") => icc = icc.or_else(|| Some(body[4..].to_vec())),
            _ => {}
        }
    }
    (nclx, icc)
}

fn parse_nclx(colr: &[u8]) -> Option<Cicp> {
    let code = |at: usize| Some(u16::from_be_bytes(colr.get(at..at + 2)?.try_into().ok()?));
    Some(Cicp {
        primaries: code(4)?,
        transfer: code(6)?,
        matrix: code(8)?,
        full_range: colr.get(10)? & 0x80 != 0,
    })
}

/// The primary image's alpha, if it has one of its size.
fn decode_alpha(items: &Items, primary: &Coded) -> Result<Option<Samples>, DecodeError> {
    let Some(alpha) = items.references_to(*b"auxl", items.primary).find(|&id| {
        // `auxC` is a full box: version and flags come before the type.
        items
            .property(id, b"auxC")
            .and_then(|aux| aux.get(4..))
            .is_some_and(|kind| kind == ALPHA_URN || kind == HEVC_ALPHA_URN)
    }) else {
        return Ok(None);
    };
    let alpha = decode_item(items, alpha)?;
    if (alpha.width(), alpha.height()) != (primary.width(), primary.height()) {
        return Ok(None);
    }
    Ok(Some(alpha.luma()?))
}

/// `rgb` with `alpha`, at `rgb`'s depth, as a fourth channel.
fn with_alpha(rgb: Samples, alpha: &Samples) -> Samples {
    let (max, alpha_max) = (u32::from(rgb.max), u32::from(alpha.max));
    let values = rgb
        .values
        .chunks_exact(3)
        .zip(&alpha.values)
        .flat_map(|(px, &a)| {
            let a = (u32::from(a) * max + alpha_max / 2) / alpha_max;
            [px[0], px[1], px[2], a as u16]
        })
        .collect();
    Samples {
        channels: 4,
        values,
        ..rgb
    }
}

/// `samples` turned and mirrored by the item's `irot` and `imir`, in the
/// order they're listed.
fn upright(items: &Items, id: u32, mut samples: Samples) -> Samples {
    for (kind, body) in items.properties(id) {
        let Some(&value) = body.first() else { continue };
        samples = match &kind {
            b"irot" => rotate(samples, value & 3),
            b"imir" => mirror(samples, value & 1 == 1),
            _ => samples,
        };
    }
    samples
}

/// `samples` turned counter-clockwise by `quarter_turns`.
fn rotate(samples: Samples, quarter_turns: u8) -> Samples {
    if quarter_turns == 0 {
        return samples;
    }
    let (width, height) = (samples.width as usize, samples.height as usize);
    let channels = samples.channels;
    let (new_width, new_height) = if quarter_turns == 2 {
        (width, height)
    } else {
        (height, width)
    };
    let mut values = Vec::with_capacity(samples.values.len());
    for y in 0..new_height {
        for x in 0..new_width {
            let (from_x, from_y) = match quarter_turns {
                1 => (width - 1 - y, x),
                2 => (width - 1 - x, height - 1 - y),
                _ => (y, height - 1 - x),
            };
            let at = (from_y * width + from_x) * channels;
            values.extend_from_slice(&samples.values[at..at + channels]);
        }
    }
    Samples {
        width: new_width as u32,
        height: new_height as u32,
        values,
        ..samples
    }
}

/// `samples` mirrored top to bottom if `vertically`, else left to right.
fn mirror(mut samples: Samples, vertically: bool) -> Samples {
    let channels = samples.channels;
    let row_len = samples.width as usize * channels;
    if vertically {
        samples.values = samples
            .values
            .chunks_exact(row_len)
            .rev()
            .flatten()
            .copied()
            .collect();
    } else {
        for row in samples.values.chunks_exact_mut(row_len) {
            let pixels: Vec<_> = row
                .chunks_exact(channels)
                .rev()
                .flatten()
                .copied()
                .collect();
            row.copy_from_slice(&pixels);
        }
    }
    samples
}

fn to_image(samples: &Samples, wide: bool) -> Result<Image<'static>, DecodeError> {
    let pixel_type = match (samples.channels, wide) {
        (1, false) => PixelType::U8,
        (3, false) => PixelType::U8x3,
        (4, false) => PixelType::U8x4,
        (1, true) => PixelType::U16,
        (3, true) => PixelType::U16x3,
        _ => PixelType::U16x4,
    };
    let bytes = if wide {
        samples.to_u16_bytes()
    } else {
        samples.to_u8()
    };
    Image::from_vec_u8(samples.width, samples.height, bytes, pixel_type)
        .map_err(|e| DecodeError::new(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        av1::{encode, encode_alpha, encode_pq},
        heif::avif::{write, write_pq},
    };

    /// 2×1 samples, then turned or mirrored: `[a, b]`.
    fn pair() -> Samples {
        Samples {
            width: 2,
            height: 1,
            channels: 1,
            max: 255,
            values: vec![1, 2],
        }
    }

    #[test]
    fn rotates_counter_clockwise() {
        // a b → b over a.
        let turned = rotate(pair(), 1);
        assert_eq!(
            (turned.width, turned.height, turned.values),
            (1, 2, vec![2, 1])
        );
        assert_eq!(rotate(pair(), 2).values, [2, 1]);
        let turned = rotate(pair(), 3);
        assert_eq!(
            (turned.width, turned.height, turned.values),
            (1, 2, vec![1, 2])
        );
    }

    #[test]
    fn mirrors_both_ways() {
        assert_eq!(mirror(pair(), false).values, [2, 1]);
        assert_eq!(mirror(pair(), true).values, [1, 2]);
        let column = rotate(pair(), 1);
        assert_eq!(mirror(column, true).values, [1, 2]);
    }

    #[test]
    fn reads_both_grid_size_widths() {
        let grid = Grid {
            rows: 2,
            columns: 3,
            width: 8160,
            height: 4590,
        };
        assert_eq!(
            parse_grid(&[0, 0, 1, 2, 0x1F, 0xE0, 0x11, 0xEE]),
            Some(grid)
        );
        let wide = parse_grid(&[0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 2]).unwrap();
        assert_eq!((wide.width, wide.height), (65536, 2));
        assert_eq!(parse_grid(&[0, 0, 1, 2, 0x1F]), None);
    }

    #[test]
    fn crops_tiles_at_the_canvas_edge() {
        // 2×2 tiles of 1..=4 on a 3×3 canvas.
        let mut canvas = Samples {
            width: 3,
            height: 3,
            channels: 1,
            max: 255,
            values: vec![0; 9],
        };
        let tile = |v| Samples {
            width: 2,
            height: 2,
            channels: 1,
            max: 255,
            values: vec![v; 4],
        };
        for (i, (left, top)) in [(0, 0), (2, 0), (0, 2), (2, 2)].into_iter().enumerate() {
            place(&mut canvas, &tile(i as u16 + 1), left, top);
        }
        assert_eq!(canvas.values, [1, 1, 2, 1, 1, 2, 3, 3, 4]);
    }

    fn rgb_image(width: u32, height: u32) -> Image<'static> {
        let rgb = (0..width * height)
            .flat_map(|i| [(i % 256) as u8, 128, 30])
            .collect();
        Image::from_vec_u8(width, height, rgb, PixelType::U8x3).unwrap()
    }

    #[test]
    fn decodes_a_pq_file_with_alpha() {
        let rgb: Vec<u8> = (0..32 * 16)
            .flat_map(|i: u16| {
                [i * 100, 30_000, 500]
                    .into_iter()
                    .flat_map(u16::to_ne_bytes)
            })
            .collect();
        let color = Image::from_vec_u8(32, 16, rgb, PixelType::U16x3).unwrap();
        let alpha: Vec<u8> = (0..32 * 16).flat_map(|_| 65535u16.to_ne_bytes()).collect();
        let alpha = Image::from_vec_u8(32, 16, alpha, PixelType::U16).unwrap();
        let file = write_pq(
            &encode_pq(&color, 90., 10).unwrap(),
            Some(&encode_alpha(&alpha, 90., 10).unwrap()),
        );

        let decoded = decode(&file, true).unwrap();
        assert_eq!(decoded.image.pixel_type(), PixelType::U16x4);
        assert_eq!(
            decoded.color,
            ColorSpace::Cicp {
                primaries: 9,
                transfer: 16
            }
        );
        let px = &decoded.image.buffer()[..8];
        assert_eq!(u16::from_ne_bytes([px[6], px[7]]), 65535);
    }

    #[test]
    fn decodes_a_gain_mapped_file() {
        let fraction = crate::gain_map::Fraction { n: 1, d: 2 };
        let channel = crate::gain_map::ChannelMapping {
            gain_map_min: fraction,
            gain_map_max: fraction,
            gamma: crate::gain_map::UFraction { n: 1, d: 1 },
            base_offset: fraction,
            alternate_offset: fraction,
        };
        let metadata = GainMapMetadata {
            use_base_color_space: true,
            base_headroom: crate::gain_map::UFraction { n: 0, d: 1 },
            alternate_headroom: crate::gain_map::UFraction { n: 2, d: 1 },
            channels: [channel; 3],
        };
        let luma = Image::from_vec_u8(16, 12, vec![200; 16 * 12], PixelType::U8).unwrap();
        let file = write(
            &encode(&rgb_image(32, 24), 90., 10).unwrap(),
            &encode(&luma, 90., 10).unwrap(),
            &metadata,
            &ColorSpace::SRGB,
        );

        let decoded = decode_gain_mapped(&file).unwrap();
        assert_eq!(decoded.metadata, metadata);
        assert_eq!(decoded.color, ColorSpace::SRGB);
        assert_eq!(
            (decoded.base.width(), decoded.base.pixel_type()),
            (32, PixelType::U8x3)
        );
        assert_eq!(
            (decoded.gain_map.width(), decoded.gain_map.pixel_type()),
            (16, PixelType::U8)
        );
        // The primary image alone, for readers without gain map support.
        assert_eq!(
            decode(&file, false).unwrap().image.pixel_type(),
            PixelType::U8x3
        );
    }
}
