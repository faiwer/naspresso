//! File encryption.

mod encrypt;
mod error;
mod password;

pub use encrypt::{EncryptedFile, encrypt};
pub use error::{EncryptError, PasswordError};
pub use password::validate_password;
