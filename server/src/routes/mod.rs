//! HTTP handlers, one module per area.

mod health;
mod router;
mod storage;

pub use router::router;
