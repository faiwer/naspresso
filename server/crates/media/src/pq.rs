//! PQ images (SMPTE ST 2084) moved into BT.2020 primaries, which PQ previews
//! are coded in.

use fast_image_resize::{PixelType, images::Image};

/// Linear-light RGB to BT.2020 RGB, by the source's CICP primaries code.
type Matrix = [[f32; 3]; 3];
const FROM_BT709: Matrix = [
    [0.6274, 0.3293, 0.0433],
    [0.0691, 0.9195, 0.0114],
    [0.0164, 0.0880, 0.8956],
];
const FROM_DISPLAY_P3: Matrix = [
    [0.7538, 0.1986, 0.0476],
    [0.0457, 0.9418, 0.0125],
    [-0.0012, 0.0176, 0.9836],
];

const M1: f32 = 2610. / 16384.;
const M2: f32 = 2523. / 4096. * 128.;
const C1: f32 = 3424. / 4096.;
const C2: f32 = 2413. / 4096. * 32.;
const C3: f32 = 2392. / 4096. * 32.;

/// Converts the 16-bit PQ `image` (`U16x3` or `U16x4`) from the CICP
/// `primaries` to BT.2020's. `false` if they aren't ones we know.
pub fn to_bt2020(image: &mut Image, primaries: u16) -> bool {
    let matrix = match primaries {
        9 => return true,
        1 => FROM_BT709,
        12 => FROM_DISPLAY_P3,
        _ => return false,
    };
    let pixel_len = match image.pixel_type() {
        PixelType::U16x3 => 6,
        PixelType::U16x4 => 8,
        _ => return false,
    };
    let linear: Vec<f32> = (0..=u16::MAX)
        .map(|v| to_linear(f32::from(v) / 65535.))
        .collect();
    for px in image.buffer_mut().chunks_exact_mut(pixel_len) {
        let rgb = [0, 2, 4].map(|i| linear[usize::from(u16::from_ne_bytes([px[i], px[i + 1]]))]);
        for (channel, row) in matrix.iter().enumerate() {
            let mixed = row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2];
            let value = (from_linear(mixed) * 65535.).round() as u16;
            px[channel * 2..channel * 2 + 2].copy_from_slice(&value.to_ne_bytes());
        }
    }
    true
}

/// PQ's EOTF: a signal in `0..=1` to light in `0..=1` of 10 000 nits.
fn to_linear(signal: f32) -> f32 {
    let p = signal.powf(1. / M2);
    ((p - C1).max(0.) / (C2 - C3 * p)).powf(1. / M1)
}

/// PQ's inverse EOTF, clamping light outside `0..=1`.
fn from_linear(light: f32) -> f32 {
    let y = light.clamp(0., 1.).powf(M1);
    ((C1 + C2 * y) / (1. + C3 * y)).powf(M2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(rgb: [u16; 3]) -> Image<'static> {
        let bytes = rgb.iter().flat_map(|v| v.to_ne_bytes()).collect();
        Image::from_vec_u8(1, 1, bytes, PixelType::U16x3).unwrap()
    }

    fn samples(image: &Image) -> Vec<u16> {
        image
            .buffer()
            .chunks_exact(2)
            .map(|v| u16::from_ne_bytes([v[0], v[1]]))
            .collect()
    }

    #[test]
    fn round_trips_the_curve() {
        for signal in [0., 0.1, 0.5, 0.58, 1.] {
            assert!((from_linear(to_linear(signal)) - signal).abs() < 1e-4);
        }
        // 100 nits sits near 0.508.
        assert!((from_linear(0.01) - 0.508).abs() < 1e-3);
    }

    #[test]
    fn keeps_gray_and_narrows_pure_colors() {
        let mut gray = image([30_000; 3]);
        assert!(to_bt2020(&mut gray, 1));
        for v in samples(&gray) {
            assert!(v.abs_diff(30_000) <= 20, "gray became {v}");
        }

        let mut red = image([40_000, 0, 0]);
        assert!(to_bt2020(&mut red, 1));
        let [r, g, b] = samples(&red)[..] else {
            unreachable!()
        };
        assert!(r < 40_000 && g > 0 && b > 0);
    }

    #[test]
    fn leaves_bt2020_and_rejects_the_unknown() {
        let mut bt2020 = image([1, 2, 3]);
        assert!(to_bt2020(&mut bt2020, 9));
        assert_eq!(samples(&bt2020), [1, 2, 3]);
        assert!(!to_bt2020(&mut image([1, 2, 3]), 5));
    }
}
