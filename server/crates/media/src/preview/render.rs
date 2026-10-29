use std::{
    fs,
    io::{BufRead, Seek},
    panic,
    path::Path,
    thread::{self, ScopedJoinHandle},
};

use fast_image_resize::{PixelType, Resizer, images::Image};
use image::{DynamicImage, ImageDecoder, ImageError, ImageReader};
use ravif::{Encoder, Img, RGB8, RGBA8};

use super::{
    PreviewError, PreviewSize,
    resize::{fit, image_size, pixels, proportional, resize, rgb},
};
use crate::{
    av1,
    gain_map::GainMapMetadata,
    heif::avif,
    jpeg::{
        self,
        decode::{Jpeg, decode as decode_jpeg},
        mpf::find_gain_map,
    },
    metadata::Hdr,
    probe::probe_file,
};

/// What the `image` crate is built to decode.
const DECODABLE: [&str; 6] = [
    "image/bmp",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/tiff",
    "image/webp",
];

/// AVIF previews of one image, one per size.
pub struct Rendered {
    avifs: Vec<(PreviewSize, Vec<u8>)>,
}

impl Rendered {
    pub fn new(avifs: Vec<(PreviewSize, Vec<u8>)>) -> Self {
        Self { avifs }
    }

    pub fn avifs(&self) -> impl Iterator<Item = (PreviewSize, &[u8])> {
        self.avifs
            .iter()
            .map(|(size, avif)| (*size, avif.as_slice()))
    }

    pub fn avif(&self, size: PreviewSize) -> Option<&[u8]> {
        self.avifs()
            .find_map(|(candidate, avif)| (candidate == size).then_some(avif))
    }
}

/// Previews of every size from the image at `path`, at AVIF `quality`
/// `1..=100` and `speed` `1..=10`, slowest to fastest. Blocks: decoding and
/// encoding are CPU-bound.
pub fn render(path: &Path, quality: f32, speed: u8) -> Result<Rendered, PreviewError> {
    let probe = probe_file(path);
    if !DECODABLE.contains(&probe.mime) {
        return Err(PreviewError::UnsupportedFormat(probe.mime));
    }
    match probe.image.map(|image| image.hdr) {
        Some(Hdr::GainMap) if probe.mime == "image/jpeg" => {
            return render_gain_mapped_jpeg(path, quality, speed);
        }
        Some(Hdr::None) | None => {}
        Some(_) => return Err(PreviewError::HdrUnsupported),
    }

    let image = decode(ImageReader::open(path)?.with_guessed_format()?)?;
    let mut current = if image.color().has_alpha() {
        let size = image_size(&image);
        pixels(image.into_rgba8().into_raw(), size, PixelType::U8x4)
    } else {
        rgb(image)
    }?;
    let mut resizer = Resizer::new();
    let mut avifs = Vec::with_capacity(PreviewSize::DESCENDING.len());
    for size in PreviewSize::DESCENDING {
        let (width, height) = fit(current.width(), current.height(), size.px());
        current = resize(&mut resizer, current, width, height)?;
        avifs.push((size, encode_avif(&current, quality, speed)?));
    }
    Ok(Rendered::new(avifs))
}

/// Gain-map AVIFs from a gain-map JPEG.
fn render_gain_mapped_jpeg(path: &Path, quality: f32, speed: u8) -> Result<Rendered, PreviewError> {
    let scaled = scale_gain_mapped_jpeg(path)?;
    // Each small encode keeps only a core or two busy, so all six run at once.
    let encoded = thread::scope(|scope| {
        let encodes: Vec<_> = scaled
            .sizes
            .iter()
            .map(|(size, base, gain_map)| {
                let base = scope.spawn(|| av1::encode(base, quality, speed));
                let gain_map = scope.spawn(|| av1::encode(gain_map, quality, speed));
                (*size, base, gain_map)
            })
            .collect();
        encodes
            .into_iter()
            .map(|(size, base, gain_map)| Ok((size, join(base)?, join(gain_map)?)))
            .collect::<Result<Vec<_>, PreviewError>>()
    })?;
    let avifs = encoded
        .iter()
        .map(|(size, base, gain_map)| {
            let avif = avif::write(base, gain_map, &scaled.metadata, scaled.icc.as_deref());
            (*size, avif)
        })
        .collect();
    Ok(Rendered::new(avifs))
}

/// The thread's result, re-raising its panic if it had one.
fn join<T>(handle: ScopedJoinHandle<'_, T>) -> T {
    handle
        .join()
        .unwrap_or_else(|panic| panic::resume_unwind(panic))
}

/// A gain-map JPEG's base image and gain map, upright, at each size.
struct GainMapped {
    metadata: GainMapMetadata,
    /// The base image's color profile.
    icc: Option<Vec<u8>>,
    /// Largest first: the size, the base image (RGB), the gain map (RGB or
    /// grayscale).
    sizes: Vec<(PreviewSize, Image<'static>, Image<'static>)>,
}

/// The base image and gain map of the gain-map JPEG at `path`, scaled to every
/// size. The gain map keeps its proportion to the base image.
fn scale_gain_mapped_jpeg(path: &Path) -> Result<GainMapped, PreviewError> {
    let file = fs::read(path)?;
    let gain_map = find_gain_map(&file).ok_or(PreviewError::HdrUnsupported)?;
    let largest = PreviewSize::DESCENDING[0].px();
    let (full_width, full_height) = jpeg::decode::size(&file)?;
    let (base, map) = thread::scope(|scope| {
        let map = scope.spawn(|| {
            decode_jpeg(gain_map.jpeg, |width, height| {
                let (base_width, base_height) = fit(full_width, full_height, largest);
                (
                    proportional(base_width, width, full_width),
                    proportional(base_height, height, full_height),
                )
            })
        });
        let base = decode_jpeg(&file, |width, height| fit(width, height, largest));
        (base, join(map))
    });
    let Jpeg {
        image: base,
        orientation,
        icc,
    } = base?;
    let mut map = map?.image;
    // The gain map carries no orientation of its own; it follows the base's.
    map.apply_orientation(orientation);

    let (base_width, base_height) = image_size(&base);
    let (map_width, map_height) = image_size(&map);
    let mut base = rgb(base)?;
    let mut map = if map.color().channel_count() == 1 {
        pixels(
            map.into_luma8().into_raw(),
            (map_width, map_height),
            PixelType::U8,
        )
    } else {
        rgb(map)
    }?;

    let mut resizer = Resizer::new();
    let mut sizes = Vec::with_capacity(PreviewSize::DESCENDING.len());
    for size in PreviewSize::DESCENDING {
        let (width, height) = fit(base_width, base_height, size.px());
        base = resize(&mut resizer, base, width, height)?;
        map = resize(
            &mut resizer,
            map,
            proportional(width, map_width, base_width),
            proportional(height, map_height, base_height),
        )?;
        sizes.push((size, base.copy(), map.copy()));
    }
    Ok(GainMapped {
        metadata: gain_map.metadata,
        icc,
        sizes,
    })
}

/// The decoded image, already upright.
fn decode<R: BufRead + Seek>(reader: ImageReader<R>) -> Result<DynamicImage, PreviewError> {
    let decode_error = |e: ImageError| PreviewError::Decode(e.to_string());
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let orientation = decoder.orientation().map_err(decode_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn encode_avif(image: &Image, quality: f32, speed: u8) -> Result<Vec<u8>, PreviewError> {
    let encoder = Encoder::new().with_quality(quality).with_speed(speed);
    let (width, height) = (image.width() as usize, image.height() as usize);
    let encoded = if image.pixel_type() == PixelType::U8x4 {
        let pixels: Vec<RGBA8> = image
            .buffer()
            .chunks_exact(4)
            .map(|px| RGBA8::new(px[0], px[1], px[2], px[3]))
            .collect();
        encoder.encode_rgba(Img::new(&pixels[..], width, height))
    } else {
        let pixels: Vec<RGB8> = image
            .buffer()
            .chunks_exact(3)
            .map(|px| RGB8::new(px[0], px[1], px[2]))
            .collect();
        encoder.encode_rgb(Img::new(&pixels[..], width, height))
    };
    encoded
        .map(|encoded| encoded.avif_file)
        .map_err(|e| PreviewError::Encode(e.to_string()))
}
