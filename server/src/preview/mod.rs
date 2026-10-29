//! AVIF previews of images under `STORAGE`, cached under `CACHE`. Knows nothing about HTTP.

mod bundle;
mod cache;

pub use cache::{Preview, PreviewCache, PreviewCacheError};
