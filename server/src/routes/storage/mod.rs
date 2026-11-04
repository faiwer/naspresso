//! `/storage/*` endpoints.

mod cursor;
mod file;
mod ls;
mod router;
mod vault;

pub use router::router;
