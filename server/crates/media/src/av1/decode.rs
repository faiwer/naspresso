//! Still AV1 frames, as AVIF items hold them, decoded by dav1d.

use dav1d::{Decoder, Picture, PixelLayout, PlanarImageComponent, Settings};

use crate::yuv::{Cicp, DecodeError, Layout, Plane, Yuv};

impl From<dav1d::Error> for DecodeError {
    fn from(e: dav1d::Error) -> Self {
        Self::new(format!("AV1: {e}"))
    }
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
    /// The color signaling of the bitstream's sequence header.
    pub fn cicp(&self) -> Cicp {
        Cicp {
            primaries: self.0.color_primaries() as u16,
            transfer: self.0.transfer_characteristic() as u16,
            matrix: self.0.matrix_coefficients() as u16,
            full_range: self.0.color_range() == dav1d::pixel::YUVRange::Full,
        }
    }

    pub fn yuv(&self) -> Yuv {
        let depth = self.0.bits_per_component().map_or(8, |bits| bits.0 as u32);
        let layout = match self.0.pixel_layout() {
            PixelLayout::I400 => Layout::Gray,
            PixelLayout::I420 => Layout::I420,
            PixelLayout::I422 => Layout::I422,
            PixelLayout::I444 => Layout::I444,
        };
        let count = if layout == Layout::Gray { 1 } else { 3 };
        let planes = PLANES[..count]
            .iter()
            .map(|&component| {
                let data = self.0.plane(component);
                let stride = self.0.stride(component);
                if depth == 8 {
                    (Plane::Narrow(data.to_vec()), stride)
                } else {
                    let values = data
                        .chunks_exact(2)
                        .map(|v| u16::from_ne_bytes([v[0], v[1]]))
                        .collect();
                    (Plane::Wide(values), stride / 2)
                }
            })
            .collect();
        Yuv {
            width: self.0.width(),
            height: self.0.height(),
            depth,
            layout,
            planes,
        }
    }
}

const PLANES: [PlanarImageComponent; 3] = [
    PlanarImageComponent::Y,
    PlanarImageComponent::U,
    PlanarImageComponent::V,
];

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
        let yuv = frame.yuv();
        assert_eq!((yuv.width, yuv.height, yuv.layout), (64, 48, Layout::I444));
        let cicp = frame.cicp();
        // dav1d reports BT.601 as 5, the same matrix as the 6 coded.
        assert_eq!((cicp.primaries, cicp.transfer, cicp.matrix), (1, 13, 5));
        assert!(cicp.full_range);

        let decoded = yuv.rgb(cicp).unwrap();
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
        let yuv = frame.yuv();
        assert_eq!(yuv.layout, Layout::Gray);
        let gray = yuv.rgb(frame.cicp()).unwrap();
        assert_eq!(gray.values.len(), 32 * 32 * 3);
        assert_eq!(yuv.luma(true).values.len(), 32 * 32);
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
        let decoded = frame.yuv().rgb(cicp).unwrap();
        let green = u16::from_ne_bytes(decoded.to_u16_bytes()[2..4].try_into().unwrap());
        assert!(green.abs_diff(30_000) < 300, "green {green}");
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"not AV1").is_err());
    }
}
