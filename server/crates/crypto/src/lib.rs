//! File encryption.

mod encrypt;
mod error;

pub use encrypt::{EncryptedFile, encrypt};
pub use error::EncryptError;
