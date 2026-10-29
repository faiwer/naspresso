//! Still images as raw AV1, for an AVIF writer to wrap: 10-bit full-range
//! 4:4:4, BT.601 YCbCr for color and luma alone for grayscale.

use std::thread;

use fast_image_resize::{PixelType, images::Image};
use rav1e::prelude::{
    ChromaSampling, ColorDescription, ColorPrimaries, Config, Context, EncoderConfig,
    EncoderStatus, Frame, FrameType, MatrixCoefficients, PixelRange, Rational,
    TransferCharacteristics, Tune,
};
use thiserror::Error;

pub const BIT_DEPTH: u8 = 10;
const MAX: f32 = ((1 << BIT_DEPTH) - 1) as f32;
/// Smaller tiles cost more in compression than they gain in parallelism.
const MIN_TILE_SIDE: usize = 128;

pub struct Av1Image {
    /// The coded frame's OBUs.
    pub data: Vec<u8>,
    /// The `AV1CodecConfigurationRecord`, for the `av1C` box.
    pub config: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub monochrome: bool,
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct EncodeError(String);

/// `image` as AV1. It must be 8-bit RGB or grayscale. `quality` is `1..=100`;
/// `speed` is `0..=10`, slowest to fastest.
pub fn encode(image: &Image, quality: f32, speed: u8) -> Result<Av1Image, EncodeError> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let monochrome = match image.pixel_type() {
        PixelType::U8x3 => false,
        PixelType::U8 => true,
        other => return Err(EncodeError(format!("can't encode {other:?} as AV1"))),
    };
    let quantizer = quality_to_quantizer(quality);
    let tiles = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(width * height / MIN_TILE_SIDE.pow(2))
        .max(1);
    let config = Config::new().with_encoder_config(EncoderConfig {
        width,
        height,
        bit_depth: usize::from(BIT_DEPTH),
        time_base: Rational::new(1, 1),
        chroma_sampling: if monochrome {
            ChromaSampling::Cs400
        } else {
            ChromaSampling::Cs444
        },
        pixel_range: PixelRange::Full,
        color_description: (!monochrome).then_some(ColorDescription {
            color_primaries: ColorPrimaries::BT709,
            transfer_characteristics: TransferCharacteristics::SRGB,
            matrix_coefficients: MatrixCoefficients::BT601,
        }),
        still_picture: true,
        quantizer,
        min_quantizer: quantizer as u8,
        tune: Tune::Psychovisual,
        tiles,
        ..EncoderConfig::with_speed_preset(speed)
    });
    let encode_error = |e: &dyn std::fmt::Display| EncodeError(e.to_string());
    let mut ctx: Context<u16> = config.new_context().map_err(|e| encode_error(&e))?;

    let mut frame = ctx.new_frame();
    if monochrome {
        let pixels: Vec<_> = image.buffer().iter().map(|&v| [to_10_bit(v)]).collect();
        fill(&mut frame, width, &pixels);
    } else {
        let pixels: Vec<_> = image
            .buffer()
            .chunks_exact(3)
            .map(|px| rgb_to_ycbcr(px[0], px[1], px[2]))
            .collect();
        fill(&mut frame, width, &pixels);
    }
    ctx.send_frame(frame).map_err(|e| encode_error(&e))?;
    ctx.flush();

    let mut data = Vec::new();
    loop {
        match ctx.receive_packet() {
            Ok(packet) if packet.frame_type == FrameType::KEY => data.extend(packet.data),
            Ok(_) => {}
            Err(EncoderStatus::Encoded | EncoderStatus::LimitReached) => break,
            Err(e) => return Err(encode_error(&e)),
        }
    }
    Ok(Av1Image {
        data,
        config: ctx.container_sequence_header(),
        width: image.width(),
        height: image.height(),
        monochrome,
    })
}

/// Copies row-major pixels, `N` samples each, into `frame`'s first `N` planes.
fn fill<const N: usize>(frame: &mut Frame<u16>, width: usize, pixels: &[[u16; N]]) {
    for (i, plane) in frame.planes[..N].iter_mut().enumerate() {
        let mut plane = plane.mut_slice(Default::default());
        for (row, pixels) in plane.rows_iter_mut().zip(pixels.chunks_exact(width)) {
            for (sample, px) in row[..width].iter_mut().zip(pixels) {
                *sample = px[i];
            }
        }
    }
}

fn to_10_bit(value: u8) -> u16 {
    (f32::from(value) * MAX / 255.).round() as u16
}

/// BT.601 full-range YCbCr, the matrix AVIF decoders assume for sRGB images.
fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> [u16; 3] {
    let scale = MAX / 255.;
    let shift = (MAX * 0.5).round();
    let (r, g, b) = (
        f32::from(r) * scale,
        f32::from(g) * scale,
        f32::from(b) * scale,
    );
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let cb = (b - y) * (0.5 / (1. - 0.114)) + shift;
    let cr = (r - y) * (0.5 / (1. - 0.299)) + shift;
    [y, cb, cr].map(|v| v.round().clamp(0., MAX) as u16)
}

/// rav1e's `0..=255` quantizer for a `1..=100` quality, as ravif maps it.
fn quality_to_quantizer(quality: f32) -> usize {
    let q = quality / 100.;
    let x = if q >= 0.82 {
        (1. - q) * 2.6
    } else if q > 0.25 {
        1. - 0.125 - 0.5 * q
    } else {
        1. - q
    };
    (x * 255.).round().clamp(0., 255.) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgb_to_full_range_ycbcr() {
        assert_eq!(rgb_to_ycbcr(0, 0, 0), [0, 512, 512]);
        assert_eq!(rgb_to_ycbcr(255, 255, 255), [1023, 512, 512]);
        let [y, cb, cr] = rgb_to_ycbcr(255, 0, 0);
        assert_eq!(y, 306);
        assert!(cb < 512 && cr == 1023);
    }

    #[test]
    fn maps_quality_like_ravif() {
        assert_eq!(quality_to_quantizer(100.), 0);
        assert_eq!(quality_to_quantizer(75.), 128);
        assert_eq!(quality_to_quantizer(1.), 252);
    }

    #[test]
    fn encodes_color_and_grayscale() {
        let rgb: Vec<u8> = (0..64 * 48)
            .flat_map(|i| [(i % 256) as u8, 128, 30])
            .collect();
        let rgb = Image::from_vec_u8(64, 48, rgb, PixelType::U8x3).unwrap();
        let color = encode(&rgb, 75., 10).unwrap();
        assert!(!color.monochrome && !color.data.is_empty());
        assert_eq!((color.width, color.height), (64, 48));
        // av1C: marker and version, then the seq_profile (1 for 4:4:4).
        assert_eq!(color.config[0], 0x81);
        assert_eq!(color.config[1] >> 5, 1);

        let luma: Vec<u8> = (0..32 * 32).map(|i| (i % 256) as u8).collect();
        let luma = Image::from_vec_u8(32, 32, luma, PixelType::U8).unwrap();
        let gray = encode(&luma, 75., 10).unwrap();
        assert!(gray.monochrome && !gray.data.is_empty());
        // av1C's mono_chrome bit.
        assert_eq!(gray.config[2] & 0x10, 0x10);
    }

    #[test]
    fn rejects_alpha() {
        let rgba = Image::new(8, 8, PixelType::U8x4);
        assert!(encode(&rgba, 75., 10).is_err());
    }
}
