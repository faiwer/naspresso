//! HTTP handlers, one module per area.

mod crypto;
mod detach;
mod health;
mod media;
mod router;
mod storage;
mod stream;

pub use router::router;
