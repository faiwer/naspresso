/// Denominator for values that come as decimals, from XMP.
const XMP_DENOMINATOR: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    pub n: i32,
    pub d: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UFraction {
    pub n: u32,
    pub d: u32,
}

/// One color channel's mapping. Min and max are log2 of the gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelMapping {
    pub gain_map_min: Fraction,
    pub gain_map_max: Fraction,
    pub gamma: UFraction,
    pub base_offset: Fraction,
    pub alternate_offset: Fraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GainMapMetadata {
    /// Whether the gain map applies in the base image's color space rather
    /// than the alternate one's.
    pub use_base_color_space: bool,
    /// Log2 of the display headroom the base image is meant for.
    pub base_headroom: UFraction,
    /// Log2 of the display headroom the alternate image is meant for.
    pub alternate_headroom: UFraction,
    /// Red, green and blue. A single-channel mapping is repeated three times.
    pub channels: [ChannelMapping; 3],
}

impl GainMapMetadata {
    /// From ISO 21496-1 binary metadata, as it follows the URN in a JPEG APP2
    /// segment. `None` if it's malformed or of a version we don't know.
    pub fn from_iso(data: &[u8]) -> Option<Self> {
        let mut data = Bytes(data);
        let minimum_version = data.u16()?;
        let _writer_version = data.u16()?;
        if minimum_version != 0 {
            return None;
        }
        let flags = data.u8()?;
        let is_multichannel = flags & 0x80 != 0;
        let use_base_color_space = flags & 0x40 != 0;
        let base_headroom = data.ufraction()?;
        let alternate_headroom = data.ufraction()?;

        let mut channels = Vec::with_capacity(3);
        for _ in 0..if is_multichannel { 3 } else { 1 } {
            channels.push(ChannelMapping {
                gain_map_min: data.fraction()?,
                gain_map_max: data.fraction()?,
                gamma: data.ufraction()?,
                base_offset: data.fraction()?,
                alternate_offset: data.fraction()?,
            });
        }
        Some(Self {
            use_base_color_space,
            base_headroom,
            alternate_headroom,
            channels: three(&channels)?,
        })
    }

    /// As ISO 21496-1 binary metadata, with one channel when all three match.
    pub fn to_iso(&self) -> Vec<u8> {
        let [r, g, b] = &self.channels;
        let channels = if r == g && g == b {
            &self.channels[..1]
        } else {
            &self.channels[..]
        };
        let mut flags = 0;
        if channels.len() == 3 {
            flags |= 0x80;
        }
        if self.use_base_color_space {
            flags |= 0x40;
        }

        // Minimum and writer versions, both 0.
        let mut out = vec![0, 0, 0, 0, flags];
        let ufraction = |out: &mut Vec<u8>, f: UFraction| {
            out.extend(f.n.to_be_bytes());
            out.extend(f.d.to_be_bytes());
        };
        ufraction(&mut out, self.base_headroom);
        ufraction(&mut out, self.alternate_headroom);
        let fraction = |out: &mut Vec<u8>, f: Fraction| {
            out.extend(f.n.to_be_bytes());
            out.extend(f.d.to_be_bytes());
        };
        for channel in channels {
            fraction(&mut out, channel.gain_map_min);
            fraction(&mut out, channel.gain_map_max);
            ufraction(&mut out, channel.gamma);
            fraction(&mut out, channel.base_offset);
            fraction(&mut out, channel.alternate_offset);
        }
        out
    }

    /// From the `hdrgm:` properties of Adobe's gain map XMP, which Ultra HDR
    /// uses too. `None` if they're missing or malformed.
    pub fn from_xmp(xmp: &[u8]) -> Option<Self> {
        let xmp = std::str::from_utf8(xmp).ok()?;
        xmp_values(xmp, "Version")?;
        let base_is_hdr = xmp_text(xmp, "BaseRenditionIsHDR")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));

        let numbers = |name, default: f64| -> Option<[f64; 3]> {
            match xmp_numbers(xmp, name) {
                None => Some([default; 3]),
                Some(values) => three(&values?),
            }
        };
        let gain_map_min = numbers("GainMapMin", 0.)?;
        let gain_map_max = numbers("GainMapMax", 1.)?;
        let gamma = numbers("Gamma", 1.)?;
        let offset_sdr = numbers("OffsetSDR", 1. / 64.)?;
        let offset_hdr = numbers("OffsetHDR", 1. / 64.)?;
        let capacity_min = numbers("HDRCapacityMin", 0.)?[0];
        let capacity_max = numbers("HDRCapacityMax", 1.)?[0];

        let (base_offset, alternate_offset, base_headroom, alternate_headroom) = if base_is_hdr {
            (offset_hdr, offset_sdr, capacity_max, capacity_min)
        } else {
            (offset_sdr, offset_hdr, capacity_min, capacity_max)
        };
        let channel = |i: usize| -> Option<ChannelMapping> {
            Some(ChannelMapping {
                gain_map_min: Fraction::from_f64(gain_map_min[i])?,
                gain_map_max: Fraction::from_f64(gain_map_max[i])?,
                gamma: UFraction::from_f64(gamma[i])?,
                base_offset: Fraction::from_f64(base_offset[i])?,
                alternate_offset: Fraction::from_f64(alternate_offset[i])?,
            })
        };
        Some(Self {
            use_base_color_space: true,
            base_headroom: UFraction::from_f64(base_headroom)?,
            alternate_headroom: UFraction::from_f64(alternate_headroom)?,
            channels: [channel(0)?, channel(1)?, channel(2)?],
        })
    }
}

impl GainMapMetadata {
    /// For a gain map over an SDR base whose values are log2 gains from 0 to
    /// `stops`, unweighted and without offsets, for a display `stops` brighter.
    pub fn from_stops(stops: f64) -> Option<Self> {
        let zero = Fraction { n: 0, d: 1 };
        let channel = ChannelMapping {
            gain_map_min: zero,
            gain_map_max: Fraction::from_f64(stops)?,
            gamma: UFraction { n: 1, d: 1 },
            base_offset: zero,
            alternate_offset: zero,
        };
        Some(Self {
            use_base_color_space: true,
            base_headroom: UFraction { n: 0, d: 1 },
            alternate_headroom: UFraction::from_f64(stops)?,
            channels: [channel; 3],
        })
    }
}

impl Fraction {
    fn from_f64(value: f64) -> Option<Self> {
        let n = (value * f64::from(XMP_DENOMINATOR)).round();
        (value.is_finite() && n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX)).then_some(
            Self {
                n: n as i32,
                d: XMP_DENOMINATOR,
            },
        )
    }
}

impl UFraction {
    fn from_f64(value: f64) -> Option<Self> {
        let n = (value * f64::from(XMP_DENOMINATOR)).round();
        (value.is_finite() && n >= 0. && n <= f64::from(u32::MAX)).then_some(Self {
            n: n as u32,
            d: XMP_DENOMINATOR,
        })
    }
}

/// One value per channel, or one for all of them.
fn three<T: Copy>(values: &[T]) -> Option<[T; 3]> {
    match *values {
        [value] => Some([value; 3]),
        [r, g, b] => Some([r, g, b]),
        _ => None,
    }
}

/// `None` when the property is absent, `Some(None)` when it's not numeric.
fn xmp_numbers(xmp: &str, name: &str) -> Option<Option<Vec<f64>>> {
    let values = xmp_values(xmp, name)?;
    Some(
        values
            .iter()
            .map(|value| value.trim().parse().ok())
            .collect(),
    )
}

fn xmp_text<'a>(xmp: &'a str, name: &str) -> Option<&'a str> {
    xmp_values(xmp, name)?.first().copied()
}

/// An `hdrgm:` property's values, from either form it comes in: an attribute,
/// `hdrgm:Gamma="1.2"`, or an element, `<hdrgm:Gamma>1.2</hdrgm:Gamma>`, whose
/// per-channel values are `<rdf:li>` items of an `<rdf:Seq>`.
fn xmp_values<'a>(xmp: &'a str, name: &str) -> Option<Vec<&'a str>> {
    for quote in ['"', '\''] {
        let attribute = format!("hdrgm:{name}={quote}");
        if let Some(start) = xmp.find(&attribute).map(|i| i + attribute.len()) {
            let len = xmp[start..].find(quote)?;
            return Some(vec![&xmp[start..start + len]]);
        }
    }

    let open = format!("<hdrgm:{name}>");
    let close = format!("</hdrgm:{name}>");
    let start = xmp.find(&open)? + open.len();
    let inner = &xmp[start..start + xmp[start..].find(&close)?];
    let items: Vec<_> = inner
        .split("<rdf:li>")
        .skip(1)
        .filter_map(|item| item.split_once("</rdf:li>").map(|(value, _)| value))
        .collect();
    Some(if items.is_empty() { vec![inner] } else { items })
}

/// Big-endian reads from the front of a slice.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|[byte]| byte)
    }

    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_be_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_be_bytes)
    }

    fn fraction(&mut self) -> Option<Fraction> {
        let n = self.take().map(i32::from_be_bytes)?;
        let d = self.u32().filter(|&d| d != 0)?;
        Some(Fraction { n, d })
    }

    fn ufraction(&mut self) -> Option<UFraction> {
        let n = self.u32()?;
        let d = self.u32().filter(|&d| d != 0)?;
        Some(UFraction { n, d })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ISO block of a real Camera Raw export, with a different mapping per channel.
    const ISO: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
        0xff, 0xfc, 0x00, 0x00, 0x80, 0x00, //
        0xff, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01, 0xfd, 0x51, 0x00, 0x00, 0x80,
        0x00, 0x10, 0x44, 0x7a, 0xb7, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, //
        0xff, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01, 0xfd, 0x51, 0x00, 0x00, 0x80,
        0x00, 0x00, 0x01, 0x75, 0x8c, 0x00, 0x00, 0x80, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, //
        0xff, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0xff, 0xfe, 0x91, 0x16, 0x00, 0x00, 0x80,
        0x00, 0x39, 0xe0, 0x5f, 0x68, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
    ];

    fn approx(fraction: Fraction) -> f64 {
        f64::from(fraction.n) / f64::from(fraction.d)
    }

    fn uapprox(fraction: UFraction) -> f64 {
        f64::from(fraction.n) / f64::from(fraction.d)
    }

    #[test]
    fn reads_iso_metadata_per_channel() {
        let meta = GainMapMetadata::from_iso(ISO).unwrap();
        assert!(meta.use_base_color_space);
        assert_eq!(meta.base_headroom, UFraction { n: 0, d: 1 });
        assert!((uapprox(meta.alternate_headroom) - 4.0).abs() < 0.001);
        let [r, g, b] = meta.channels;
        assert_eq!(approx(r.gain_map_min), -4.0);
        assert!((approx(r.gain_map_max) - 3.979).abs() < 0.001);
        assert!((approx(b.gain_map_max) + 2.866).abs() < 0.001);
        assert!((uapprox(r.gamma) - 0.254).abs() < 0.001);
        assert!((uapprox(g.gamma) - 2.918).abs() < 0.001);
        assert_eq!(approx(r.base_offset), 1. / 64.);
    }

    #[test]
    fn writes_iso_metadata_back() {
        assert_eq!(GainMapMetadata::from_iso(ISO).unwrap().to_iso(), ISO);

        let xmp = br#"hdrgm:Version="1.0" hdrgm:GainMapMax="2.5""#;
        let single = GainMapMetadata::from_xmp(xmp).unwrap();
        let iso = single.to_iso();
        assert_eq!(iso[4], 0x40);
        assert_eq!(iso.len(), 5 + 16 + 40);
        assert_eq!(GainMapMetadata::from_iso(&iso), Some(single));
    }

    #[test]
    fn rejects_truncated_or_unknown_iso_metadata() {
        assert!(GainMapMetadata::from_iso(&ISO[..ISO.len() - 1]).is_none());
        let mut newer = ISO.to_vec();
        newer[1] = 1;
        assert!(GainMapMetadata::from_iso(&newer).is_none());
    }

    #[test]
    fn reads_xmp_attributes_and_sequences() {
        let xmp = br#"<rdf:Description hdrgm:Version="1.0" hdrgm:HDRCapacityMax="1.33"
            hdrgm:OffsetSDR="0.015625">
            <hdrgm:GainMapMax><rdf:Seq><rdf:li>1.69</rdf:li><rdf:li>1.67</rdf:li>
            <rdf:li>1.64</rdf:li></rdf:Seq></hdrgm:GainMapMax>
            <hdrgm:Gamma>1.5</hdrgm:Gamma></rdf:Description>"#;
        let meta = GainMapMetadata::from_xmp(xmp).unwrap();
        assert!((uapprox(meta.alternate_headroom) - 1.33).abs() < 1e-6);
        assert_eq!(meta.base_headroom.n, 0);
        let [r, _, b] = meta.channels;
        assert!((approx(r.gain_map_max) - 1.69).abs() < 1e-6);
        assert!((approx(b.gain_map_max) - 1.64).abs() < 1e-6);
        assert!((uapprox(b.gamma) - 1.5).abs() < 1e-6);
        assert_eq!(approx(r.gain_map_min), 0.);
        assert!((approx(r.alternate_offset) - 1. / 64.).abs() < 1e-6);
    }

    #[test]
    fn swaps_roles_when_the_base_is_hdr() {
        let xmp = br#"hdrgm:Version="1.0" hdrgm:BaseRenditionIsHDR="True"
            hdrgm:HDRCapacityMax="2" hdrgm:OffsetSDR="0.1" hdrgm:OffsetHDR="0.2""#;
        let meta = GainMapMetadata::from_xmp(xmp).unwrap();
        assert!((uapprox(meta.base_headroom) - 2.).abs() < 1e-6);
        assert!((approx(meta.channels[0].base_offset) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn rejects_xmp_without_a_gain_map() {
        assert!(GainMapMetadata::from_xmp(br#"xmp:Rating="5""#).is_none());
        let two_channels = br#"hdrgm:Version="1.0"
            <hdrgm:Gamma><rdf:Seq><rdf:li>1</rdf:li><rdf:li>2</rdf:li></rdf:Seq></hdrgm:Gamma>"#;
        assert!(GainMapMetadata::from_xmp(two_channels).is_none());
    }
}
