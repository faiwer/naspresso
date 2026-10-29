//! Coding-independent code points (ITU-T H.273): the color signaling AVIF,
//! HEIF and PNG share.

use crate::metadata::Hdr;

/// SMPTE ST 2084.
pub const TRANSFER_PQ: u16 = 16;
/// ARIB STD-B67.
pub const TRANSFER_HLG: u16 = 18;

pub fn hdr_from_transfer(transfer: u16) -> Hdr {
    match transfer {
        TRANSFER_PQ => Hdr::Pq,
        TRANSFER_HLG => Hdr::Hlg,
        _ => Hdr::None,
    }
}
