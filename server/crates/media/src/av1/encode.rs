//! Still images as raw AV1, for an AVIF writer to wrap: 10-bit full-range
//! 4:4:4. SDR is BT.601 YCbCr for color and luma alone for grayscale; PQ is
//! BT.2020 YCbCr.

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

/// Luma weights of red and blue.
type Matrix = (f32, f32);
const BT601: Matrix = (0.299, 0.114);
const BT2020: Matrix = (0.2627, 0.0593);

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

/// `image` as AV1. It must be 8-bit sRGB or grayscale. `quality` is
/// `1..=100`; `speed` is `0..=10`, slowest to fastest.
pub fn encode(image: &Image, quality: f32, speed: u8) -> Result<Av1Image, EncodeError> {
    let buffer = image.buffer();
    match image.pixel_type() {
        PixelType::U8x3 => {
            let pixels: Vec<_> = buffer
                .chunks_exact(3)
                .map(|px| to_ycbcr([px[0], px[1], px[2]].map(|v| f32::from(v) / 255.), BT601))
                .collect();
            let color = ColorDescription {
                color_primaries: ColorPrimaries::BT709,
                transfer_characteristics: TransferCharacteristics::SRGB,
                matrix_coefficients: MatrixCoefficients::BT601,
            };
            encode_pixels(image, Some(color), &pixels, quality, speed)
        }
        PixelType::U8 => {
            let pixels: Vec<_> = buffer.iter().map(|&v| [to_10_bit(v)]).collect();
            encode_pixels(image, None, &pixels, quality, speed)
        }
        other => Err(EncodeError(format!("can't encode {other:?} as SDR AV1"))),
    }
}

/// `image` as AV1. It must be 16-bit BT.2100 PQ RGB. `quality` and `speed`
/// are as in [`encode`].
pub fn encode_pq(image: &Image, quality: f32, speed: u8) -> Result<Av1Image, EncodeError> {
    if image.pixel_type() != PixelType::U16x3 {
        let pixel_type = image.pixel_type();
        return Err(EncodeError(format!(
            "can't encode {pixel_type:?} as PQ AV1"
        )));
    }
    let pixels: Vec<_> = image
        .buffer()
        .chunks_exact(6)
        .map(|px| {
            let sample = |i: usize| f32::from(u16::from_ne_bytes([px[i], px[i + 1]])) / 65535.;
            to_ycbcr([sample(0), sample(2), sample(4)], BT2020)
        })
        .collect();
    let color = ColorDescription {
        color_primaries: ColorPrimaries::BT2020,
        transfer_characteristics: TransferCharacteristics::SMPTE2084,
        matrix_coefficients: MatrixCoefficients::BT2020NCL,
    };
    encode_pixels(image, Some(color), &pixels, quality, speed)
}

/// The 8- or 16-bit alpha channel `image` as monochrome AV1, for an AVIF
/// alpha item. `quality` and `speed` are as in [`encode`].
pub fn encode_alpha(image: &Image, quality: f32, speed: u8) -> Result<Av1Image, EncodeError> {
    let pixels: Vec<_> = match image.pixel_type() {
        PixelType::U8 => image.buffer().iter().map(|&v| [to_10_bit(v)]).collect(),
        PixelType::U16 => image
            .buffer()
            .chunks_exact(2)
            .map(|v| [(f32::from(u16::from_ne_bytes([v[0], v[1]])) * MAX / 65535.).round() as u16])
            .collect(),
        pixel_type => {
            return Err(EncodeError(format!(
                "can't encode {pixel_type:?} as AV1 alpha"
            )));
        }
    };
    encode_pixels(image, None, &pixels, quality, speed)
}

/// Encodes 10-bit pixels of `N` samples: luma alone, or YCbCr in `color`.
fn encode_pixels<const N: usize>(
    image: &Image,
    color: Option<ColorDescription>,
    pixels: &[[u16; N]],
    quality: f32,
    speed: u8,
) -> Result<Av1Image, EncodeError> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let monochrome = N == 1;
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
        color_description: color,
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
    fill(&mut frame, width, pixels);
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

/// Full-range 10-bit YCbCr of nonlinear RGB in `0..=1`, by `matrix`.
fn to_ycbcr(rgb: [f32; 3], (kr, kb): Matrix) -> [u16; 3] {
    let [r, g, b] = rgb.map(|v| v * MAX);
    let shift = (MAX * 0.5).round();
    let y = kr * r + (1. - kr - kb) * g + kb * b;
    let cb = (b - y) * (0.5 / (1. - kb)) + shift;
    let cr = (r - y) * (0.5 / (1. - kr)) + shift;
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

    fn ycbcr(rgb: [u8; 3], matrix: Matrix) -> [u16; 3] {
        to_ycbcr(rgb.map(|v| f32::from(v) / 255.), matrix)
    }

    #[test]
    fn converts_rgb_to_full_range_ycbcr() {
        assert_eq!(ycbcr([0, 0, 0], BT601), [0, 512, 512]);
        assert_eq!(ycbcr([255, 255, 255], BT601), [1023, 512, 512]);
        let [y, cb, cr] = ycbcr([255, 0, 0], BT601);
        assert_eq!(y, 306);
        assert!(cb < 512 && cr == 1023);
        assert_eq!(ycbcr([255, 0, 0], BT2020)[0], 269);
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
    fn encodes_pq() {
        let rgb: Vec<u8> = (0..32 * 16)
            .flat_map(|i: u16| {
                [i * 100, 30_000, 500]
                    .into_iter()
                    .flat_map(u16::to_ne_bytes)
            })
            .collect();
        let rgb = Image::from_vec_u8(32, 16, rgb, PixelType::U16x3).unwrap();
        let pq = encode_pq(&rgb, 75., 10).unwrap();
        assert!(!pq.monochrome && !pq.data.is_empty());
    }

    #[test]
    fn encodes_alpha() {
        let alpha: Vec<u8> = (0..32 * 16)
            .flat_map(|i: u16| (i * 128).to_ne_bytes())
            .collect();
        let alpha = Image::from_vec_u8(32, 16, alpha, PixelType::U16).unwrap();
        let alpha = encode_alpha(&alpha, 75., 10).unwrap();
        assert!(alpha.monochrome && !alpha.data.is_empty());
        assert!(encode_alpha(&Image::new(8, 8, PixelType::U8), 75., 10).is_ok());
        assert!(encode_alpha(&Image::new(8, 8, PixelType::U8x3), 75., 10).is_err());
    }

    #[test]
    fn rejects_alpha() {
        let rgba = Image::new(8, 8, PixelType::U8x4);
        assert!(encode(&rgba, 75., 10).is_err());
        let rgba = Image::new(8, 8, PixelType::U16x4);
        assert!(encode_pq(&rgba, 75., 10).is_err());
    }
}
