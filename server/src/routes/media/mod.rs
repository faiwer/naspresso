//! `/media/*` endpoints: derived views of files under `STORAGE`.

mod preview;
mod router;

pub use router::router;
