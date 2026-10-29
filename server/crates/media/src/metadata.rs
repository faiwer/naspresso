use serde::Serialize;

/// What a listing knows about an image from its headers, without decoding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImageMetadata {
    /// Pixels, upright: as shown once the orientation is applied.
    pub width: u32,
    pub height: u32,
    pub hdr: Hdr,
    /// Whether it has frames to play: a GIF, APNG, WebP or JPEG XL animation,
    /// or a HEIF or AVIF sequence. Previews show the first frame.
    pub animated: bool,
}

/// How an image carries high dynamic range, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Hdr {
    None,
    /// Maybe HDR, but finding out costs more than reading headers: a JPEG XL
    /// that packs a JPEG, whose gain map would be inside the packed data.
    Unknown,
    /// An SDR image plus a gain map to brighten it on HDR displays: Ultra HDR,
    /// ISO 21496-1 or Apple's.
    GainMap,
    /// The PQ transfer function (SMPTE ST 2084).
    Pq,
    /// The hybrid log-gamma transfer function.
    Hlg,
}
