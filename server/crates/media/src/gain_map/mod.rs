//! Gain map metadata (ISO 21496-1): how the gain map turns the base image into
//! the alternate one, and for which display headrooms.

mod metadata;

pub use metadata::GainMapMetadata;
#[cfg(test)]
pub use metadata::{ChannelMapping, Fraction, UFraction};
