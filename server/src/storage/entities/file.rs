use chrono::{
    DateTime, Utc,
    serde::{ts_milliseconds, ts_milliseconds_option},
};
use media::ImageMetadata;
use serde::Serialize;

#[derive(Serialize)]
pub struct File {
    pub name: String,
    #[serde(with = "ts_milliseconds")]
    pub modified_at: DateTime<Utc>,
    /// `None` when the filesystem doesn't record it.
    #[serde(with = "ts_milliseconds_option")]
    pub created_at: Option<DateTime<Utc>>,
    /// Bytes.
    pub size: u64,
    /// By magic bytes for images, by extension for everything else.
    pub mime: &'static str,
    /// `None` for non-images and for images whose headers couldn't be read.
    pub image: Option<ImageMetadata>,
    /// Whether previews of the current version are cached, so asking for one
    /// won't wait for a render.
    pub has_preview: bool,
}
