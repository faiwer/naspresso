//! Reversible filename encryption.

mod base62;
mod error;
mod ff1;
mod name;
mod pad;

pub use error::FilenameError;
pub use name::{EXTENSION, decrypt_filename, encrypt_filename};
