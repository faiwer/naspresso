//! Planar YCbCr pictures, as video codecs decode them, turned into RGB or
//! luma samples.

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

/// How the chroma planes are sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Luma alone.
    Gray,
    I420,
    I422,
    I444,
}

/// A plane's samples: bytes at 8 bits, else 16-bit values.
pub enum Plane {
    Narrow(Vec<u8>),
    Wide(Vec<u16>),
}

/// A decoded picture.
pub struct Yuv {
    pub width: u32,
    pub height: u32,
    /// 8, 10 or 12.
    pub depth: u32,
    pub layout: Layout,
    /// Y, then U and V unless gray, each with its row stride in samples.
    pub planes: Vec<(Plane, u32)>,
}

/// GBR: the planes are green, blue and red.
const MATRIX_IDENTITY: u16 = 0;

impl Yuv {
    /// RGB by `cicp`'s matrix and range; gray widened to RGB.
    pub fn rgb(&self, cicp: Cicp) -> Result<Samples, DecodeError> {
        let (width, height) = (self.width, self.height);
        let max = (1u16 << self.depth) - 1;
        let samples = |values| Samples {
            width,
            height,
            channels: 3,
            max,
            values,
        };
        if self.layout == Layout::Gray {
            let luma = self.luma(cicp.full_range);
            return Ok(samples(luma.values.iter().flat_map(|&v| [v; 3]).collect()));
        }
        if cicp.matrix == MATRIX_IDENTITY {
            if self.layout != Layout::I444 {
                return Err(DecodeError::new("subsampled GBR"));
            }
            // Every GBR plane takes luma's range.
            let [y, u, v] = [0, 1, 2].map(|plane| self.plane(plane, cicp.full_range));
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
        match &self.planes[..] {
            [
                (Plane::Narrow(y), y_stride),
                (Plane::Narrow(u), u_stride),
                (Plane::Narrow(v), v_stride),
            ] => {
                let planar = YuvPlanarImage {
                    y_plane: y,
                    y_stride: *y_stride,
                    u_plane: u,
                    u_stride: *u_stride,
                    v_plane: v,
                    v_stride: *v_stride,
                    width,
                    height,
                };
                let mut rgb = vec![0; len];
                match self.layout {
                    Layout::I420 => yuv420_to_rgb(&planar, &mut rgb, stride, range, matrix),
                    Layout::I422 => yuv422_to_rgb(&planar, &mut rgb, stride, range, matrix),
                    _ => yuv444_to_rgb(&planar, &mut rgb, stride, range, matrix),
                }?;
                Ok(samples(rgb.into_iter().map(u16::from).collect()))
            }
            [
                (Plane::Wide(y), y_stride),
                (Plane::Wide(u), u_stride),
                (Plane::Wide(v), v_stride),
            ] => {
                let planar = YuvPlanarImage {
                    y_plane: y,
                    y_stride: *y_stride,
                    u_plane: u,
                    u_stride: *u_stride,
                    v_plane: v,
                    v_stride: *v_stride,
                    width,
                    height,
                };
                let mut rgb = vec![0; len];
                match (self.layout, self.depth) {
                    (Layout::I420, 10) => i010_to_rgb10(&planar, &mut rgb, stride, range, matrix),
                    (Layout::I420, _) => i012_to_rgb12(&planar, &mut rgb, stride, range, matrix),
                    (Layout::I422, 10) => i210_to_rgb10(&planar, &mut rgb, stride, range, matrix),
                    (Layout::I422, _) => i212_to_rgb12(&planar, &mut rgb, stride, range, matrix),
                    (_, 10) => i410_to_rgb10(&planar, &mut rgb, stride, range, matrix),
                    (_, _) => i412_to_rgb12(&planar, &mut rgb, stride, range, matrix),
                }?;
                Ok(samples(rgb))
            }
            _ => Err(DecodeError::new("YUV planes of mixed depths")),
        }
    }

    /// Luma alone, full range: alpha, or a grayscale image.
    pub fn luma(&self, full_range: bool) -> Samples {
        Samples {
            width: self.width,
            height: self.height,
            channels: 1,
            max: (1 << self.depth) - 1,
            values: self.plane(0, full_range),
        }
    }

    /// The samples of the `index`th plane, row by row without padding,
    /// widened from luma's limited range unless `full_range`.
    fn plane(&self, index: usize, full_range: bool) -> Vec<u16> {
        let (plane, stride) = &self.planes[index];
        let (width, height) = (self.width as usize, self.height as usize);
        let mut samples: Vec<u16> = match plane {
            Plane::Narrow(values) => rows(values, *stride, width, height)
                .map(u16::from)
                .collect(),
            Plane::Wide(values) => rows(values, *stride, width, height).collect(),
        };
        if !full_range {
            let max = f32::from((1u16 << self.depth) - 1);
            let black = f32::from(16u16 << (self.depth - 8));
            let span = f32::from(219u16 << (self.depth - 8));
            for v in &mut samples {
                *v = ((f32::from(*v) - black) * max / span)
                    .round()
                    .clamp(0., max) as u16;
            }
        }
        samples
    }
}

/// The first `width` values of each of `height` rows, `stride` apart.
fn rows<T: Copy>(
    values: &[T],
    stride: u32,
    width: usize,
    height: usize,
) -> impl Iterator<Item = T> + '_ {
    values
        .chunks(stride as usize)
        .take(height)
        .flat_map(move |row| row[..width].iter().copied())
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn strips_padding_and_expands_limited_range() {
        // 2×2 luma in rows of 3: 16 is black and 235 white in limited range.
        let yuv = Yuv {
            width: 2,
            height: 2,
            depth: 8,
            layout: Layout::Gray,
            planes: vec![(Plane::Narrow(vec![16, 235, 0, 235, 16, 0]), 3)],
        };
        assert_eq!(yuv.luma(true).values, [16, 235, 235, 16]);
        assert_eq!(yuv.luma(false).values, [0, 255, 255, 0]);
        let full = Cicp {
            primaries: 1,
            transfer: 13,
            matrix: 6,
            full_range: true,
        };
        assert_eq!(
            yuv.rgb(full).unwrap().values[..6],
            [16, 16, 16, 235, 235, 235]
        );
    }
}
