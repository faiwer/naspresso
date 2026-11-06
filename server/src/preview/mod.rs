//! AVIF previews of images under `STORAGE`, cached under `CACHE`. Knows nothing about HTTP.

mod bundle;
mod cache;

pub use cache::{OpenNpe, Preview, PreviewCache, PreviewCacheError};
