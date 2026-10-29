//! HTTP handlers, one module per area.

mod health;
mod router;
mod storage;
mod stream;

pub use router::router;
