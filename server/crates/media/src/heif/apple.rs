//! Apple's gain maps from before ISO 21496-1 (iOS 14.1 to 17): an auxiliary
//! image of gains for linear light, whose headroom only the EXIF maker note
//! gives.

use crate::yuv::Samples;

pub const GAIN_MAP_URN: &[u8] = b"urn:com:apple:photo:2020:aux:hdrgainmap\0";

const EXIF_IFD_TAG: u16 = 0x8769;
const MAKER_NOTE_TAG: u16 = 0x927C;
const HEADROOM_TAG: u16 = 0x0021;
const GAIN_TAG: u16 = 0x0030;
const MAKER_NOTE_HEADER: &[u8] = b"Apple iOS\0";
/// The header, a version, then the byte order; the directory follows.
const MAKER_NOTE_IFD: usize = 14;

/// How many stops brighter than SDR the photo's HDR rendition goes, from the
/// HEIF Exif item's payload, by Apple's formula for maker note tags 33 and 48.
/// `None` if they're missing or give no headroom.
pub fn headroom_stops(exif_item: &[u8]) -> Option<f64> {
    // The payload starts with the offset to the TIFF header past it.
    let offset = u32::from_be_bytes(exif_item.get(..4)?.try_into().ok()?) as usize;
    let tiff = Tiff::new(exif_item.get(4 + offset..)?)?;
    let first = tiff.u32(4)? as usize;
    let exif = tiff.u32(tiff.entry(first, EXIF_IFD_TAG)?.2)? as usize;
    let (_, len, value_at) = tiff.entry(exif, MAKER_NOTE_TAG)?;
    let start = tiff.u32(value_at)? as usize;
    let note = tiff.data.get(start..start.checked_add(len as usize)?)?;
    if !note.starts_with(MAKER_NOTE_HEADER) {
        return None;
    }
    let note = Tiff {
        data: note,
        little_endian: note.get(12..14)? == b"II",
    };
    let headroom = note.rational(MAKER_NOTE_IFD, HEADROOM_TAG)?;
    let gain = note.rational(MAKER_NOTE_IFD, GAIN_TAG)?;
    let stops = match (headroom < 1., gain <= 0.01) {
        (true, true) => -20. * gain + 1.8,
        (true, false) => -0.101 * gain + 1.601,
        (false, true) => -70. * gain + 3.,
        (false, false) => -0.303 * gain + 2.303,
    };
    // Less than a hundredth of a stop isn't HDR.
    (stops > 0.01).then_some(stops)
}

/// Turns Apple's gains into ISO ones for `GainMapMetadata::from_stops(stops)`.
/// Apple scales linear light by `1 + (2^stops - 1) × g`, `g` being the gain
/// decoded by the sRGB curve; ISO by 2 to the power of the gain.
pub fn to_iso_gains(map: &mut Samples, stops: f64) {
    let headroom = stops.exp2();
    let max = f64::from(map.max);
    let table: Vec<u16> = (0..=map.max)
        .map(|value| {
            let gain = srgb_to_linear(f64::from(value) / max);
            ((1. + (headroom - 1.) * gain).log2() / stops * max).round() as u16
        })
        .collect();
    for value in &mut map.values {
        *value = table[usize::from(*value)];
    }
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// A TIFF structure held in memory, with offsets from its start.
struct Tiff<'a> {
    data: &'a [u8],
    little_endian: bool,
}

impl<'a> Tiff<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let little_endian = match data.get(..4)? {
            b"II*\0" => true,
            b"MM\0*" => false,
            _ => return None,
        };
        Some(Self {
            data,
            little_endian,
        })
    }

    fn u16(&self, at: usize) -> Option<u16> {
        let bytes = self.data.get(at..at + 2)?.try_into().ok()?;
        Some(if self.little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let bytes = self.data.get(at..at + 4)?.try_into().ok()?;
        Some(if self.little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }

    /// The type, count and value field position of `tag` in the directory
    /// at `ifd`.
    fn entry(&self, ifd: usize, tag: u16) -> Option<(u16, u32, usize)> {
        let count = usize::from(self.u16(ifd)?);
        (0..count).find_map(|i| {
            let at = ifd + 2 + i * 12;
            (self.u16(at)? == tag).then_some(())?;
            Some((self.u16(at + 2)?, self.u32(at + 4)?, at + 8))
        })
    }

    /// The value of a single RATIONAL or SRATIONAL `tag`.
    fn rational(&self, ifd: usize, tag: u16) -> Option<f64> {
        const RATIONAL: u16 = 5;
        const SRATIONAL: u16 = 10;
        let (kind, 1, value_at) = self.entry(ifd, tag)? else {
            return None;
        };
        let at = self.u32(value_at)? as usize;
        let (n, d) = (self.u32(at)?, self.u32(at + 4)?);
        let (n, d) = match kind {
            RATIONAL => (f64::from(n), f64::from(d)),
            SRATIONAL => (f64::from(n as i32), f64::from(d as i32)),
            _ => return None,
        };
        (d != 0.).then(|| n / d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A HEIF Exif payload whose maker note gives `headroom` and `gain` as
    /// SRATIONALs over 1000, all big-endian.
    fn exif(headroom: i32, gain: i32) -> Vec<u8> {
        let mut note = MAKER_NOTE_HEADER.to_vec();
        note.extend([0, 1, b'M', b'M']);
        note.extend(2u16.to_be_bytes());
        let values = (MAKER_NOTE_IFD + 2 + 2 * 12 + 4) as u32;
        for (i, tag) in [HEADROOM_TAG, GAIN_TAG].into_iter().enumerate() {
            note.extend(tag.to_be_bytes());
            note.extend(10u16.to_be_bytes());
            note.extend(1u32.to_be_bytes());
            note.extend((values + i as u32 * 8).to_be_bytes());
        }
        note.extend([0; 4]);
        for value in [headroom, gain] {
            note.extend(value.to_be_bytes());
            note.extend(1000u32.to_be_bytes());
        }

        // Header, IFD0 at 8 with the EXIF IFD pointer, the EXIF IFD at 26
        // with the maker note, which starts at 44.
        let mut tiff = b"MM\0*".to_vec();
        tiff.extend(8u32.to_be_bytes());
        tiff.extend(1u16.to_be_bytes());
        tiff.extend(EXIF_IFD_TAG.to_be_bytes());
        tiff.extend(4u16.to_be_bytes());
        tiff.extend(1u32.to_be_bytes());
        tiff.extend(26u32.to_be_bytes());
        tiff.extend([0; 4]);
        tiff.extend(1u16.to_be_bytes());
        tiff.extend(MAKER_NOTE_TAG.to_be_bytes());
        tiff.extend(7u16.to_be_bytes());
        tiff.extend((note.len() as u32).to_be_bytes());
        tiff.extend(44u32.to_be_bytes());
        tiff.extend([0; 4]);
        tiff.extend(note);

        let mut payload = 6u32.to_be_bytes().to_vec();
        payload.extend(b"Exif\0\0");
        payload.extend(tiff);
        payload
    }

    #[test]
    fn reads_the_headroom_by_apples_formula() {
        let stops = headroom_stops(&exif(1010, 5)).unwrap();
        assert!((stops - (3. - 70. * 0.005)).abs() < 1e-9, "{stops}");
        let stops = headroom_stops(&exif(500, 200)).unwrap();
        assert!((stops - (1.601 - 0.101 * 0.2)).abs() < 1e-9, "{stops}");
    }

    #[test]
    fn needs_the_maker_note() {
        let mut payload = exif(1010, 5);
        // The maker note is the last 60 bytes.
        let at = payload.len() - 60;
        payload[at..at + 5].copy_from_slice(b"Nokia");
        assert_eq!(headroom_stops(&payload), None);
        assert_eq!(headroom_stops(&payload[..40]), None);
        // Thirty stops of gain leave no headroom.
        assert_eq!(headroom_stops(&exif(1010, 30_000)), None);
    }

    #[test]
    fn keeps_the_ends_and_lifts_the_middle() {
        let mut map = Samples {
            width: 3,
            height: 1,
            channels: 1,
            max: 255,
            values: vec![0, 128, 255],
        };
        to_iso_gains(&mut map, 2.);
        // Half the sRGB range is a fifth of the linear gain: log2(1 + 3 × 0.216) / 2.
        assert_eq!(map.values, [0, 92, 255]);
    }
}
