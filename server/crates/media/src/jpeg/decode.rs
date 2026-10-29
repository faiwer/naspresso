//! JPEG decoding with libjpeg-turbo, at reduced size when that's enough.

use image::{DynamicImage, ImageBuffer, metadata::Orientation};
use thiserror::Error;
use turbojpeg::{Colorspace, ScalingFactor};

#[derive(Debug, Error)]
#[error("{0}")]
pub struct DecodeError(String);

impl From<turbojpeg::Error> for DecodeError {
    fn from(e: turbojpeg::Error) -> Self {
        Self(e.to_string())
    }
}

impl From<jpeg_decoder::Error> for DecodeError {
    fn from(e: jpeg_decoder::Error) -> Self {
        Self(e.to_string())
    }
}

pub struct Jpeg {
    /// Already upright.
    pub image: DynamicImage,
    /// What `image` was turned from.
    pub orientation: Orientation,
    pub icc: Option<Vec<u8>>,
}

/// The JPEG's stored size, before turning upright.
pub fn size(jpeg: &[u8]) -> Result<(u32, u32), DecodeError> {
    let header = turbojpeg::read_header(jpeg)?;
    Ok((side(header.width), side(header.height)))
}

/// A JPEG decoded at the smallest of 1/8, 1/4, 1/2 and full size that still
/// covers what `needed` asks for, given the full size. Much faster than
/// decoding in full: the reduced inverse DCT skips most of the work.
pub fn decode(
    jpeg: &[u8],
    needed: impl FnOnce(u32, u32) -> (u32, u32),
) -> Result<Jpeg, DecodeError> {
    let mut decompressor = turbojpeg::Decompressor::new()?;
    let header = decompressor.read_header(jpeg)?;
    let (format, channels) = match header.colorspace {
        Colorspace::Gray => (turbojpeg::PixelFormat::GRAY, 1),
        Colorspace::YCbCr | Colorspace::RGB => (turbojpeg::PixelFormat::RGB, 3),
        other => return Err(DecodeError(format!("can't decode {other:?} JPEGs"))),
    };
    let (needed_width, needed_height) = needed(side(header.width), side(header.height));
    let factor = [
        ScalingFactor::ONE_EIGHTH,
        ScalingFactor::ONE_QUARTER,
        ScalingFactor::ONE_HALF,
    ]
    .into_iter()
    .find(|factor| {
        side(factor.scale(header.width)) >= needed_width
            && side(factor.scale(header.height)) >= needed_height
    })
    .unwrap_or(ScalingFactor::ONE);
    decompressor.set_scaling_factor(factor)?;

    let header = header.scaled(factor);
    let mut pixels = vec![0; header.width * header.height * channels];
    let output = turbojpeg::Image {
        pixels: &mut pixels[..],
        width: header.width,
        pitch: header.width * channels,
        height: header.height,
        format,
    };
    decompressor.decompress(jpeg, output)?;
    let (width, height) = (side(header.width), side(header.height));
    let image = if channels == 1 {
        ImageBuffer::from_raw(width, height, pixels).map(DynamicImage::ImageLuma8)
    } else {
        ImageBuffer::from_raw(width, height, pixels).map(DynamicImage::ImageRgb8)
    };
    let mut image =
        image.ok_or_else(|| DecodeError("JPEG pixels don't match its size".to_owned()))?;

    // TurboJPEG doesn't expose these; this stops reading at the image data.
    let mut metadata = jpeg_decoder::Decoder::new(jpeg);
    metadata.read_info()?;
    let orientation = metadata
        .exif_data()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    image.apply_orientation(orientation);
    Ok(Jpeg {
        image,
        orientation,
        icc: metadata.icc_profile(),
    })
}

fn side(side: usize) -> u32 {
    u32::try_from(side).unwrap_or(u32::MAX)
}
