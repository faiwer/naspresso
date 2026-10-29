//! Segment markers and identifiers shared by the JPEG walkers.

pub const SOI: u8 = 0xD8;
pub const EOI: u8 = 0xD9;
pub const SOS: u8 = 0xDA;
pub const APP1: u8 = 0xE1;
pub const APP2: u8 = 0xE2;

/// Opens an XMP APP1 segment.
pub const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// Opens an ISO 21496-1 gain-map APP2 segment.
pub const ISO_GAIN_MAP_ID: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

/// Markers without a length or body.
pub fn is_standalone(marker: u8) -> bool {
    matches!(marker, 0x01 | 0xD0..=0xD7 | SOI)
}
