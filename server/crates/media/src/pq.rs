//! 16-bit HDR images moved into BT.2100 PQ, the encoding PQ previews are coded
//! in: from PQ or HLG, in BT.2020, BT.709 or Display P3 primaries.

use fast_image_resize::{PixelType, images::Image};

use crate::cicp::{TRANSFER_HLG, TRANSFER_PQ};

/// Linear-light RGB to BT.2020 RGB.
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

const HLG_A: f32 = 0.178_832_77;
const HLG_B: f32 = 0.284_668_92;
const HLG_C: f32 = 0.559_910_7;
/// HLG is shown as on BT.2100's reference display: 1000 nits peak, which sets
/// its system gamma to 1.2.
const HLG_PEAK_NITS: f32 = 1000.;
const HLG_GAMMA: f32 = 1.2;
const PQ_PEAK_NITS: f32 = 10_000.;

/// Converts the 16-bit `image` (`U16x3` or `U16x4`), coded with the CICP
/// `primaries` and `transfer`, to BT.2100 PQ. `false` if they aren't ones we
/// know.
pub fn to_bt2100_pq(image: &mut Image, primaries: u16, transfer: u16) -> bool {
    let matrix = match primaries {
        9 => None,
        1 => Some(FROM_BT709),
        12 => Some(FROM_DISPLAY_P3),
        _ => return false,
    };
    let hlg = match transfer {
        TRANSFER_PQ => false,
        TRANSFER_HLG => true,
        _ => return false,
    };
    let pixel_len = match image.pixel_type() {
        PixelType::U16x3 => 6,
        PixelType::U16x4 => 8,
        _ => return false,
    };
    if matrix.is_none() && !hlg {
        return true;
    }
    for px in image.buffer_mut().chunks_exact_mut(pixel_len) {
        let signal = [0, 2, 4].map(|i| f32::from(u16::from_ne_bytes([px[i], px[i + 1]])) / 65535.);
        let mut light = signal.map(if hlg { hlg_to_scene } else { pq_to_linear });
        if let Some(matrix) = matrix {
            light = matrix.map(|row| row[0] * light[0] + row[1] * light[1] + row[2] * light[2]);
        }
        if hlg {
            light = hlg_to_display(light);
        }
        for (channel, light) in light.into_iter().enumerate() {
            let value = (linear_to_pq(light) * 65535.).round() as u16;
            px[channel * 2..channel * 2 + 2].copy_from_slice(&value.to_ne_bytes());
        }
    }
    true
}

/// PQ's EOTF: a signal in `0..=1` to light in `0..=1` of 10 000 nits.
fn pq_to_linear(signal: f32) -> f32 {
    let p = signal.powf(1. / M2);
    ((p - C1).max(0.) / (C2 - C3 * p)).powf(1. / M1)
}

/// PQ's inverse EOTF, clamping light outside `0..=1`.
fn linear_to_pq(light: f32) -> f32 {
    let y = light.clamp(0., 1.).powf(M1);
    ((C1 + C2 * y) / (1. + C3 * y)).powf(M2)
}

/// HLG's inverse OETF: a signal in `0..=1` to scene light in `0..=1`.
fn hlg_to_scene(signal: f32) -> f32 {
    if signal <= 0.5 {
        signal * signal / 3.
    } else {
        (((signal - HLG_C) / HLG_A).exp() + HLG_B) / 12.
    }
}

/// HLG's OOTF: BT.2020 scene light to display light, as a fraction of PQ's
/// peak.
fn hlg_to_display(scene: [f32; 3]) -> [f32; 3] {
    let luma = 0.2627 * scene[0] + 0.6780 * scene[1] + 0.0593 * scene[2];
    let gain = HLG_PEAK_NITS / PQ_PEAK_NITS * luma.max(0.).powf(HLG_GAMMA - 1.);
    scene.map(|v| v * gain)
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

    /// The nits of a converted 16-bit PQ sample.
    fn nits(sample: u16) -> f32 {
        pq_to_linear(f32::from(sample) / 65535.) * PQ_PEAK_NITS
    }

    #[test]
    fn round_trips_the_curve() {
        for signal in [0., 0.1, 0.5, 0.58, 1.] {
            assert!((linear_to_pq(pq_to_linear(signal)) - signal).abs() < 1e-4);
        }
        // 100 nits sits near 0.508.
        assert!((linear_to_pq(0.01) - 0.508).abs() < 1e-3);
    }

    #[test]
    fn keeps_gray_and_narrows_pure_colors() {
        let mut gray = image([30_000; 3]);
        assert!(to_bt2100_pq(&mut gray, 1, TRANSFER_PQ));
        for v in samples(&gray) {
            assert!(v.abs_diff(30_000) <= 20, "gray became {v}");
        }

        let mut red = image([40_000, 0, 0]);
        assert!(to_bt2100_pq(&mut red, 1, TRANSFER_PQ));
        let [r, g, b] = samples(&red)[..] else {
            unreachable!()
        };
        assert!(r < 40_000 && g > 0 && b > 0);
    }

    #[test]
    fn shows_hlg_as_on_a_1000_nit_display() {
        let mut white = image([65535; 3]);
        assert!(to_bt2100_pq(&mut white, 9, TRANSFER_HLG));
        for v in samples(&white) {
            assert!((nits(v) - 1000.).abs() < 5., "peak at {} nits", nits(v));
        }
        // BT.2408: HLG's reference white, 75%, shows at 203 nits.
        let mut reference = image([(0.75f32 * 65535.).round() as u16; 3]);
        assert!(to_bt2100_pq(&mut reference, 9, TRANSFER_HLG));
        let shown = nits(samples(&reference)[0]);
        assert!((shown - 203.).abs() < 2., "reference white at {shown} nits");
    }

    #[test]
    fn leaves_bt2100_pq_and_rejects_the_unknown() {
        let mut bt2100 = image([1, 2, 3]);
        assert!(to_bt2100_pq(&mut bt2100, 9, TRANSFER_PQ));
        assert_eq!(samples(&bt2100), [1, 2, 3]);
        assert!(!to_bt2100_pq(&mut image([1, 2, 3]), 5, TRANSFER_PQ));
        assert!(!to_bt2100_pq(&mut image([1, 2, 3]), 9, 13));
    }
}
