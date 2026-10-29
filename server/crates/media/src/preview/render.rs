use std::{
    fs,
    io::{BufRead, BufReader, Cursor, Seek},
    panic,
    path::Path,
    thread::{self, ScopedJoinHandle},
};

use fast_image_resize::{PixelType, Resizer, images::Image};
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader};
use ravif::{Encoder, Img, RGB8, RGBA8};

use super::{
    PreviewError, PreviewSize,
    resize::{fit, image_size, pixels, proportional, resize, rgb},
};
use crate::{
    av1,
    gain_map::GainMapMetadata,
    heif::{
        avif::{self, BaseColor},
        decode::{decode as decode_avif, decode_gain_mapped as decode_gain_mapped_avif},
    },
    jpeg::{
        self,
        decode::{Jpeg, decode as decode_jpeg},
        mpf::find_gain_map,
    },
    jxl::{
        decode::{Output, decode as decode_jxl, unpack_jpeg},
        gain_map::decode_gain_mapped,
    },
    metadata::Hdr,
    pq,
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
    let hdr = probe.image.map(|image| image.hdr);
    if probe.mime == "image/jxl" {
        return make_previews_from_jxl(&fs::read(path)?, hdr, quality, speed);
    }
    if probe.mime == "image/avif" {
        return make_previews_from_avif(&fs::read(path)?, hdr, quality, speed);
    }
    if !DECODABLE.contains(&probe.mime) {
        return Err(PreviewError::UnsupportedFormat(probe.mime));
    }
    match hdr {
        Some(Hdr::GainMap) if probe.mime == "image/jpeg" => {
            return make_previews_from_gain_mapped_jpeg(&fs::read(path)?, quality, speed);
        }
        Some(Hdr::None) | None => {}
        Some(_) => return Err(PreviewError::HdrUnsupported),
    }

    let image = decode(ImageReader::open(path)?.with_guessed_format()?)?;
    encode_sdr_avifs(sdr_pixels(image)?, quality, speed)
}

/// `image` as 8-bit RGB, or RGBA if it has alpha.
fn sdr_pixels(image: DynamicImage) -> Result<Image<'static>, PreviewError> {
    if image.color().has_alpha() {
        let size = image_size(&image);
        pixels(image.into_rgba8().into_raw(), size, PixelType::U8x4)
    } else {
        rgb(image)
    }
}

/// SDR AVIFs from an 8-bit sRGB image, with or without alpha.
fn encode_sdr_avifs(
    image: Image<'static>,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
    let avifs = scale(image)?
        .into_iter()
        .map(|(size, image)| Ok((size, encode_avif(&image, quality, speed)?)))
        .collect::<Result<_, PreviewError>>()?;
    Ok(Rendered::new(avifs))
}

/// PQ AVIFs from a 16-bit BT.2100 PQ image, with or without alpha.
fn encode_pq_avifs(
    image: Image<'static>,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
    let scaled: Vec<_> = scale(image)?
        .into_iter()
        .map(|(size, image)| Ok((size, split_alpha(image)?)))
        .collect::<Result<_, PreviewError>>()?;
    // Edges show alpha artifacts more than color ones, so alpha gets more bits, as in ravif.
    let alpha_quality = (quality + 100.) / 2.;
    // Each small encode keeps only a core or two busy, so all of them run at once.
    let encoded = thread::scope(|scope| {
        let encodes: Vec<_> = scaled
            .iter()
            .map(|(size, (color, alpha))| {
                let color = scope.spawn(|| av1::encode_pq(color, quality, speed));
                let alpha = alpha.as_ref().map(|alpha| {
                    scope.spawn(move || av1::encode_alpha(alpha, alpha_quality, speed))
                });
                (*size, color, alpha)
            })
            .collect();
        encodes
            .into_iter()
            .map(|(size, color, alpha)| {
                let alpha = alpha.map(join).transpose()?;
                Ok((size, avif::write_pq(&join(color)?, alpha.as_ref())))
            })
            .collect::<Result<_, PreviewError>>()
    })?;
    Ok(Rendered::new(encoded))
}

/// A 16-bit RGB image, and its alpha channel apart if it has one.
fn split_alpha(
    image: Image<'static>,
) -> Result<(Image<'static>, Option<Image<'static>>), PreviewError> {
    if image.pixel_type() != PixelType::U16x4 {
        return Ok((image, None));
    }
    let size = (image.width(), image.height());
    let (color, alpha): (Vec<_>, Vec<_>) = image
        .buffer()
        .chunks_exact(8)
        .map(|px| (<[u8; 6]>::try_from(&px[..6]).unwrap(), [px[6], px[7]]))
        .unzip();
    Ok((
        pixels(color.concat(), size, PixelType::U16x3)?,
        Some(pixels(alpha.concat(), size, PixelType::U16)?),
    ))
}

/// `image` at every size, largest first.
fn scale(mut image: Image<'static>) -> Result<Vec<(PreviewSize, Image<'static>)>, PreviewError> {
    let mut resizer = Resizer::new();
    let mut sizes = Vec::with_capacity(PreviewSize::DESCENDING.len());
    for size in PreviewSize::DESCENDING {
        let (width, height) = fit(image.width(), image.height(), size.px());
        image = resize(&mut resizer, image, width, height)?;
        sizes.push((size, image.copy()));
    }
    Ok(sizes)
}

/// Previews of a JPEG XL `file`: PQ for PQ and HLG images, gain-map AVIFs for
/// an SDR image with a gain map, those of the JPEG for a packed JPEG, SDR for
/// the rest.
fn make_previews_from_jxl(
    file: &[u8],
    hdr: Option<Hdr>,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
    match hdr {
        Some(Hdr::Unknown) => make_previews_from_jpeg(&unpack_jpeg(file)?, quality, speed),
        Some(Hdr::Pq | Hdr::Hlg) => encode_pq_avifs(decode_jxl(file, Output::Pq)?, quality, speed),
        Some(Hdr::None) | None => encode_sdr_avifs(decode_jxl(file, Output::Srgb)?, quality, speed),
        Some(Hdr::GainMap) => {
            let decoded = decode_gain_mapped(file)?;
            let scaled = scale_gain_mapped(
                decoded.metadata,
                BaseColor::Icc(decoded.icc),
                decoded.base,
                decoded.gain_map,
            )?;
            encode_gain_mapped_avifs(&scaled, quality, speed)
        }
    }
}

/// Previews of an AVIF `file`: PQ for a PQ image, gain-map AVIFs for an SDR
/// image with a gain map, SDR for the rest.
fn make_previews_from_avif(
    file: &[u8],
    hdr: Option<Hdr>,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
    match hdr {
        Some(Hdr::Pq) => {
            let mut decoded = decode_avif(file, true)?;
            match decoded.color {
                BaseColor::Cicp {
                    primaries,
                    transfer: 16,
                } if pq::to_bt2020(&mut decoded.image, primaries) => {}
                _ => return Err(PreviewError::HdrUnsupported),
            }
            encode_pq_avifs(decoded.image, quality, speed)
        }
        Some(Hdr::GainMap) => {
            let decoded = decode_gain_mapped_avif(file)?;
            let scaled = scale_gain_mapped(
                decoded.metadata,
                decoded.color,
                decoded.base,
                decoded.gain_map,
            )?;
            encode_gain_mapped_avifs(&scaled, quality, speed)
        }
        Some(Hdr::None) | None => encode_sdr_avifs(decode_avif(file, false)?.image, quality, speed),
        Some(Hdr::Hlg | Hdr::Unknown) => Err(PreviewError::HdrUnsupported),
    }
}

/// Gain-map AVIFs from a gain-map JPEG, SDR ones from any other.
fn make_previews_from_jpeg(jpeg: &[u8], quality: f32, speed: u8) -> Result<Rendered, PreviewError> {
    let probe = jpeg::probe::read(&mut BufReader::new(Cursor::new(jpeg)))?;
    if probe.hdr == Hdr::GainMap {
        return make_previews_from_gain_mapped_jpeg(jpeg, quality, speed);
    }
    let image = decode(ImageReader::with_format(
        Cursor::new(jpeg),
        ImageFormat::Jpeg,
    ))?;
    encode_sdr_avifs(sdr_pixels(image)?, quality, speed)
}

/// Gain-map AVIFs from the gain-map JPEG `file`.
fn make_previews_from_gain_mapped_jpeg(
    file: &[u8],
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
    encode_gain_mapped_avifs(&scale_gain_mapped_jpeg(file)?, quality, speed)
}

/// Gain-map AVIFs, one per size.
fn encode_gain_mapped_avifs(
    scaled: &GainMapped,
    quality: f32,
    speed: u8,
) -> Result<Rendered, PreviewError> {
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
            let avif = avif::write(base, gain_map, &scaled.metadata, &scaled.color);
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

/// A base image and its gain map, upright, at each size.
struct GainMapped {
    metadata: GainMapMetadata,
    /// The base image's color space.
    color: BaseColor,
    /// Largest first: the size, the base image (RGB), the gain map (RGB or
    /// grayscale).
    sizes: Vec<(PreviewSize, Image<'static>, Image<'static>)>,
}

/// The base image and gain map of the gain-map JPEG `file`, scaled to every
/// size. The gain map keeps its proportion to the base image.
fn scale_gain_mapped_jpeg(file: &[u8]) -> Result<GainMapped, PreviewError> {
    let gain_map = find_gain_map(file).ok_or(PreviewError::HdrUnsupported)?;
    let largest = PreviewSize::DESCENDING[0].px();
    let (full_width, full_height) = jpeg::decode::size(file)?;
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
        let base = decode_jpeg(file, |width, height| fit(width, height, largest));
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

    let map = if map.color().channel_count() == 1 {
        let size = image_size(&map);
        pixels(map.into_luma8().into_raw(), size, PixelType::U8)
    } else {
        rgb(map)
    }?;
    let color = icc.map_or(BaseColor::SRGB, BaseColor::Icc);
    scale_gain_mapped(gain_map.metadata, color, rgb(base)?, map)
}

/// `base` and its gain map `map` scaled to every size, the gain map keeping
/// its proportion to the base image.
fn scale_gain_mapped(
    metadata: GainMapMetadata,
    color: BaseColor,
    mut base: Image<'static>,
    mut map: Image<'static>,
) -> Result<GainMapped, PreviewError> {
    let (base_width, base_height) = (base.width(), base.height());
    let (map_width, map_height) = (map.width(), map.height());
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
        metadata,
        color,
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
