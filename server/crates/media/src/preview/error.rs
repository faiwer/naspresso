use std::io;

use thiserror::Error;

use crate::{av1, jpeg};

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("preview size must be 128, 256 or 512, got {0}")]
    InvalidSize(u32),

    #[error("can't make a preview of {0}")]
    UnsupportedFormat(&'static str),

    #[error("previews of this kind of HDR image aren't supported yet")]
    HdrUnsupported,

    #[error("can't decode the image: {0}")]
    Decode(String),

    #[error("can't encode the preview: {0}")]
    Encode(String),

    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<jpeg::decode::DecodeError> for PreviewError {
    fn from(e: jpeg::decode::DecodeError) -> Self {
        Self::Decode(e.to_string())
    }
}

impl From<av1::EncodeError> for PreviewError {
    fn from(e: av1::EncodeError) -> Self {
        Self::Encode(e.to_string())
    }
}
