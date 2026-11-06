//! Previews: an image as AVIFs at a few sizes, for thumbnails and viewing.
//! Doesn't cache them.

mod error;
mod render;
mod resize;
mod size;

pub use error::PreviewError;
pub use render::{Rendered, render, render_reader};
pub use size::PreviewSize;
