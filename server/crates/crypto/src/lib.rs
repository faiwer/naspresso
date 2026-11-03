//! File encryption.

mod encrypt;
mod error;
mod filename;
mod password;

pub use encrypt::{EncryptedFile, encrypt};
pub use error::{EncryptError, PasswordError};
pub use filename::{EXTENSION, FilenameError, decrypt_filename, encrypt_filename};
pub use password::{Key, prepare, validate_password};
