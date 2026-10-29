//! Still AV1 frames, as AVIF items hold them, decoded by dav1d and turned into
//! RGB or luma samples.

use dav1d::{Decoder, Picture, PixelLayout, PlanarImageComponent, Settings};
use thiserror::Error;
use yuvutils_rs::{
    YuvError, YuvPlanarImage, YuvRange, YuvStandardMatrix, i010_to_rgb10, i012_to_rgb12,
    i210_to_rgb10, i212_to_rgb12, i410_to_rgb10, i412_to_rgb12, yuv420_to_rgb, yuv422_to_rgb,
    yuv444_to_rgb,
};

#[derive(Debug, Error)]
#[error("{0}")]
pub struct DecodeError(String);

impl DecodeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<dav1d::Error> for DecodeError {
    fn from(e: dav1d::Error) -> Self {
        Self(format!("AV1: {e}"))
    }
}

impl From<YuvError> for DecodeError {
    fn from(e: YuvError) -> Self {
        Self(format!("YUV: {e}"))
    }
}

/// CICP codes (ITU-T H.273) for how samples map to colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cicp {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// Samples of `channels` interleaved channels, each in `0..=max`.
pub struct Samples {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub max: u16,
    pub values: Vec<u16>,
}

/// A decoded frame.
pub struct Frame(Picture);

/// The frame the OBUs in `data` code.
pub fn decode(data: &[u8]) -> Result<Frame, DecodeError> {
    let mut settings = Settings::new();
    settings.set_max_frame_delay(1);
    let mut decoder = Decoder::with_settings(&settings)?;
    let mut pending = match decoder.send_data(data.to_vec(), None, None, None) {
        Ok(()) => false,
        Err(e) if e.is_again() => true,
        Err(e) => return Err(e.into()),
    };
    loop {
        match decoder.get_picture() {
            Ok(picture) => return Ok(Frame(picture)),
            Err(e) if e.is_again() && pending => {
                pending = match decoder.send_pending_data() {
                    Ok(()) => false,
                    Err(e) if e.is_again() => true,
                    Err(e) => return Err(e.into()),
                };
            }
            Err(e) if e.is_again() => return Err(DecodeError::new("AV1 without a frame")),
            Err(e) => return Err(e.into()),
        }
    }
}

impl Frame {
    pub fn width(&self) -> u32 {
        self.0.width()
    }

    pub fn height(&self) -> u32 {
        self.0.height()
    }

    /// The color signaling of the bitstream's sequence header.
    pub fn cicp(&self) -> Cicp {
        Cicp {
            primaries: self.0.color_primaries() as u16,
            transfer: self.0.transfer_characteristic() as u16,
            matrix: self.0.matrix_coefficients() as u16,
            full_range: self.0.color_range() == dav1d::pixel::YUVRange::Full,
        }
    }

    /// Whether only luma is coded.
    pub fn monochrome(&self) -> bool {
        self.0.pixel_layout() == PixelLayout::I400
    }

    /// RGB by `cicp`'s matrix and range; gray widened to RGB.
    pub fn rgb(&self, cicp: Cicp) -> Result<Samples, DecodeError> {
        let (width, height) = (self.width(), self.height());
        let depth = self.bit_depth();
        let max = (1u16 << depth) - 1;
        let samples = |values| Samples {
            width,
            height,
            channels: 3,
            max,
            values,
        };
        if self.monochrome() {
            let luma = self.luma(cicp.full_range);
            return Ok(samples(luma.values.iter().flat_map(|&v| [v; 3]).collect()));
        }
        let layout = self.0.pixel_layout();
        if cicp.matrix == MATRIX_IDENTITY {
            if layout != PixelLayout::I444 {
                return Err(DecodeError::new("subsampled GBR"));
            }
            // Every GBR plane takes luma's range.
            let [y, u, v] = PLANES.map(|plane| self.plane(plane, cicp.full_range));
            let rgb = (0..y.len()).flat_map(|i| [v[i], y[i], u[i]]).collect();
            return Ok(samples(rgb));
        }

        let range = if cicp.full_range {
            YuvRange::Full
        } else {
            YuvRange::Limited
        };
        let matrix = standard_matrix(cicp.matrix)?;
        let len = width as usize * height as usize * 3;
        let stride = width * 3;
        if depth == 8 {
            let [y, u, v] = PLANES.map(|plane| self.0.plane(plane));
            let planar = YuvPlanarImage {
                y_plane: &y,
                y_stride: self.0.stride(PlanarImageComponent::Y),
                u_plane: &u,
                u_stride: self.0.stride(PlanarImageComponent::U),
                v_plane: &v,
                v_stride: self.0.stride(PlanarImageComponent::V),
                width,
                height,
            };
            let mut rgb = vec![0; len];
            match layout {
                PixelLayout::I420 => yuv420_to_rgb(&planar, &mut rgb, stride, range, matrix),
                PixelLayout::I422 => yuv422_to_rgb(&planar, &mut rgb, stride, range, matrix),
                _ => yuv444_to_rgb(&planar, &mut rgb, stride, range, matrix),
            }?;
            return Ok(samples(rgb.into_iter().map(u16::from).collect()));
        }

        let [(y, y_stride), (u, u_stride), (v, v_stride)] =
            PLANES.map(|plane| self.wide_plane(plane));
        let planar = YuvPlanarImage {
            y_plane: &y,
            y_stride,
            u_plane: &u,
            u_stride,
            v_plane: &v,
            v_stride,
            width,
            height,
        };
        let mut rgb = vec![0; len];
        match (layout, depth) {
            (PixelLayout::I420, 10) => i010_to_rgb10(&planar, &mut rgb, stride, range, matrix),
            (PixelLayout::I420, _) => i012_to_rgb12(&planar, &mut rgb, stride, range, matrix),
            (PixelLayout::I422, 10) => i210_to_rgb10(&planar, &mut rgb, stride, range, matrix),
            (PixelLayout::I422, _) => i212_to_rgb12(&planar, &mut rgb, stride, range, matrix),
            (_, 10) => i410_to_rgb10(&planar, &mut rgb, stride, range, matrix),
            (_, _) => i412_to_rgb12(&planar, &mut rgb, stride, range, matrix),
        }?;
        Ok(samples(rgb))
    }

    /// Luma alone, full range: alpha, or a grayscale image.
    pub fn luma(&self, full_range: bool) -> Samples {
        let depth = self.bit_depth();
        Samples {
            width: self.width(),
            height: self.height(),
            channels: 1,
            max: (1 << depth) - 1,
            values: self.plane(PlanarImageComponent::Y, full_range),
        }
    }

    /// 8, 10 or 12.
    fn bit_depth(&self) -> u32 {
        self.0.bits_per_component().map_or(8, |bits| bits.0 as u32)
    }

    /// The samples of `plane`, row by row without padding, widened from luma's
    /// limited range unless `full_range`.
    fn plane(&self, plane: PlanarImageComponent, full_range: bool) -> Vec<u16> {
        let depth = self.bit_depth();
        let (values, stride) = if depth == 8 {
            let data = self.0.plane(plane);
            (
                data.iter().copied().map(u16::from).collect(),
                self.0.stride(plane),
            )
        } else {
            self.wide_plane(plane)
        };
        let (width, height) = (self.width() as usize, self.height() as usize);
        let rows = values.chunks(stride as usize).take(height);
        let mut samples: Vec<u16> = rows.flat_map(|row| row[..width].iter().copied()).collect();
        if !full_range {
            let max = f32::from((1u16 << depth) - 1);
            let black = f32::from(16u16 << (depth - 8));
            let span = f32::from(219u16 << (depth - 8));
            for v in &mut samples {
                *v = ((f32::from(*v) - black) * max / span)
                    .round()
                    .clamp(0., max) as u16;
            }
        }
        samples
    }

    /// A 16-bit plane's samples and its stride in samples.
    fn wide_plane(&self, plane: PlanarImageComponent) -> (Vec<u16>, u32) {
        let data = self.0.plane(plane);
        let samples = data
            .chunks_exact(2)
            .map(|v| u16::from_ne_bytes([v[0], v[1]]))
            .collect();
        (samples, self.0.stride(plane) / 2)
    }
}

const PLANES: [PlanarImageComponent; 3] = [
    PlanarImageComponent::Y,
    PlanarImageComponent::U,
    PlanarImageComponent::V,
];

/// GBR: the planes are green, blue and red.
const MATRIX_IDENTITY: u16 = 0;

fn standard_matrix(code: u16) -> Result<YuvStandardMatrix, DecodeError> {
    Ok(match code {
        1 => YuvStandardMatrix::Bt709,
        // BT.601, and what decoders assume when it's unspecified.
        2 | 5 | 6 => YuvStandardMatrix::Bt601,
        4 => YuvStandardMatrix::Custom(0.30, 0.11),
        7 => YuvStandardMatrix::Smpte240,
        // Constant luminance decodes close enough as non-constant for a preview.
        9 | 10 => YuvStandardMatrix::Bt2020,
        other => {
            return Err(DecodeError(format!(
                "unsupported matrix coefficients {other}"
            )));
        }
    })
}

impl Samples {
    /// As 8-bit samples.
    pub fn to_u8(&self) -> Vec<u8> {
        let max = u32::from(self.max);
        self.values
            .iter()
            .map(|&v| ((u32::from(v) * 255 + max / 2) / max) as u8)
            .collect()
    }

    /// As 16-bit native-endian samples, spanning `0..=65535`.
    pub fn to_u16_bytes(&self) -> Vec<u8> {
        let max = u32::from(self.max);
        self.values
            .iter()
            .flat_map(|&v| (((u32::from(v) * 65535 + max / 2) / max) as u16).to_ne_bytes())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use fast_image_resize::{PixelType, images::Image};

    use super::*;
    use crate::av1::{encode, encode_pq};

    fn gradient(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| [(i % 256) as u8, 128, (255 - i % 256) as u8])
            .collect()
    }

    #[test]
    fn decodes_what_the_encoder_wrote() {
        let rgb = gradient(64, 48);
        let image = Image::from_vec_u8(64, 48, rgb.clone(), PixelType::U8x3).unwrap();
        let frame = decode(&encode(&image, 100., 10).unwrap().data).unwrap();
        assert_eq!((frame.width(), frame.height()), (64, 48));
        assert!(!frame.monochrome());
        let cicp = frame.cicp();
        // dav1d reports BT.601 as 5, the same matrix as the 6 coded.
        assert_eq!((cicp.primaries, cicp.transfer, cicp.matrix), (1, 13, 5));
        assert!(cicp.full_range);

        let decoded = frame.rgb(cicp).unwrap();
        assert_eq!((decoded.channels, decoded.max), (3, 1023));
        let decoded = decoded.to_u8();
        let worst = rgb
            .iter()
            .zip(&decoded)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(worst <= 8, "off by {worst}");
    }

    #[test]
    fn decodes_luma_alone() {
        let luma: Vec<u8> = (0..32 * 32).map(|i| (i % 256) as u8).collect();
        let image = Image::from_vec_u8(32, 32, luma, PixelType::U8).unwrap();
        let frame = decode(&encode(&image, 100., 10).unwrap().data).unwrap();
        assert!(frame.monochrome());
        let gray = frame.rgb(frame.cicp()).unwrap();
        assert_eq!(gray.values.len(), 32 * 32 * 3);
        assert_eq!(frame.luma(true).values.len(), 32 * 32);
    }

    #[test]
    fn decodes_pq_wide() {
        let rgb: Vec<u8> = (0..32 * 16)
            .flat_map(|i: u16| {
                [i * 100, 30_000, 500]
                    .into_iter()
                    .flat_map(u16::to_ne_bytes)
            })
            .collect();
        let image = Image::from_vec_u8(32, 16, rgb, PixelType::U16x3).unwrap();
        let frame = decode(&encode_pq(&image, 100., 10).unwrap().data).unwrap();
        let cicp = frame.cicp();
        assert_eq!((cicp.primaries, cicp.transfer, cicp.matrix), (9, 16, 9));
        let decoded = frame.rgb(cicp).unwrap();
        let green = u16::from_ne_bytes(decoded.to_u16_bytes()[2..4].try_into().unwrap());
        assert!(green.abs_diff(30_000) < 300, "green {green}");
    }

    #[test]
    fn scales_samples_to_8_and_16_bits() {
        let samples = Samples {
            width: 1,
            height: 1,
            channels: 1,
            max: 255,
            values: vec![255],
        };
        assert_eq!(samples.to_u8(), [255]);
        assert_eq!(samples.to_u16_bytes(), 65535u16.to_ne_bytes());
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"not AV1").is_err());
    }
}
