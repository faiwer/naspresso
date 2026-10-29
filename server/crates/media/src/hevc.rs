//! Still HEVC pictures, as HEIC items hold them.

use libde265_rs::{Channel, ChromaFormat, DeError, DecodeResult, Image, new_decoder};

use crate::yuv::{DecodeError, Layout, Plane, Yuv};

const VPS: u8 = 32;
const PPS: u8 = 34;

impl From<DeError> for DecodeError {
    fn from(error: DeError) -> Self {
        Self::new(format!("HEVC: {error}"))
    }
}

/// The picture an item's `hvcC` configuration and data code.
pub fn decode(config: &[u8], data: &[u8]) -> Result<Yuv, DecodeError> {
    let (length_size, mut parameter_sets) =
        parse_config(config).ok_or_else(|| DecodeError::new("unreadable hvcC"))?;
    parameter_sets.sort_by_key(|unit| nal_type(unit));
    let units = split(data, length_size).ok_or_else(|| DecodeError::new("HEVC data cut short"))?;

    let (mut input, mut output) = new_decoder()?;
    for unit in parameter_sets.into_iter().chain(units) {
        if !unit.is_empty() {
            input.push_nal(unit, 0, 0)?;
        }
    }
    input.flush_data()?;
    loop {
        let result = input.decode()?;
        if let Some(image) = output.next_picture() {
            return to_yuv(&image);
        }
        if let DecodeResult::Done = result {
            return Err(DecodeError::new("HEVC data holds no picture"));
        }
    }
}

fn to_yuv(image: &Image) -> Result<Yuv, DecodeError> {
    let layout = match image.chroma_format() {
        ChromaFormat::Mono => Layout::Gray,
        ChromaFormat::C420 => Layout::I420,
        ChromaFormat::C422 => Layout::I422,
        ChromaFormat::C444 => Layout::I444,
    };
    let channels: &[Channel] = if layout == Layout::Gray {
        &[Channel::Y]
    } else {
        &[Channel::Y, Channel::Cb, Channel::Cr]
    };
    let depth = image.bits_per_pixel(Channel::Y);
    let mut planes = Vec::with_capacity(channels.len());
    for &channel in channels {
        if image.bits_per_pixel(channel) != depth {
            return Err(DecodeError::new("HEVC planes differ in bit depth"));
        }
        let (bytes, stride) = image.plane(channel);
        planes.push(if depth == 8 {
            (Plane::Narrow(bytes.to_vec()), stride as u32)
        } else {
            let values = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                .collect();
            (Plane::Wide(values), stride as u32 / 2)
        });
    }
    Ok(Yuv {
        width: image.width(Channel::Y),
        height: image.height(Channel::Y),
        depth,
        layout,
        planes,
    })
}

/// The NAL unit length size and the parameter sets of an
/// HEVCDecoderConfigurationRecord (ISO/IEC 14496-15).
fn parse_config(config: &[u8]) -> Option<(usize, Vec<&[u8]>)> {
    let length_size = usize::from(config.get(21)? & 3) + 1;
    let arrays = *config.get(22)?;
    let mut at = 23;
    let mut sets = Vec::new();
    for _ in 0..arrays {
        let kind = config.get(at)? & 0x3F;
        let count = be_u16(config, at + 1)?;
        at += 3;
        for _ in 0..count {
            let len = usize::from(be_u16(config, at)?);
            let unit = config.get(at + 2..at + 2 + len)?;
            if (VPS..=PPS).contains(&kind) {
                sets.push(unit);
            }
            at += 2 + len;
        }
    }
    Some((length_size, sets))
}

/// The NAL units of `data`, each after a `length_size`-byte length.
fn split(data: &[u8], length_size: usize) -> Option<Vec<&[u8]>> {
    let mut units = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let len = rest
            .get(..length_size)?
            .iter()
            .fold(0, |n, &b| n << 8 | usize::from(b));
        units.push(rest.get(length_size..length_size + len)?);
        rest = &rest[length_size + len..];
    }
    Some(units)
}

fn nal_type(unit: &[u8]) -> u8 {
    unit.first().map_or(u8::MAX, |b| b >> 1 & 0x3F)
}

fn be_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `hvcC` with 4-byte lengths and the given NAL arrays.
    fn config(arrays: &[(u8, &[&[u8]])]) -> Vec<u8> {
        let mut config = vec![0; 21];
        config.push(3);
        config.push(arrays.len() as u8);
        for (kind, units) in arrays {
            config.push(0x80 | kind);
            config.extend_from_slice(&(units.len() as u16).to_be_bytes());
            for unit in *units {
                config.extend_from_slice(&(unit.len() as u16).to_be_bytes());
                config.extend_from_slice(unit);
            }
        }
        config
    }

    #[test]
    fn reads_parameter_sets_and_skips_sei() {
        let sei: &[u8] = &[39 << 1, 1];
        let (vps, sps, pps): (&[u8], &[u8], &[u8]) = (&[VPS << 1, 1], &[33 << 1, 1], &[PPS << 1]);
        let config = config(&[(VPS, &[vps]), (33, &[sps]), (PPS, &[pps]), (39, &[sei])]);
        assert_eq!(parse_config(&config), Some((4, vec![vps, sps, pps])));
        assert_eq!(parse_config(&config[..config.len() - 1]), None);
    }

    #[test]
    fn splits_length_prefixed_units() {
        let data = [0, 2, 1, 2, 0, 1, 3];
        assert_eq!(split(&data, 2), Some(vec![&[1, 2][..], &[3]]));
        assert_eq!(split(&data[..6], 2), None);
    }
}
