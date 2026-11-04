//! File encryption.

mod content;
mod encrypt;
mod error;
mod filename;
mod password;

pub use content::{
    AsyncContentFile, ContentError, ContentFile, decrypt_content, encrypt_content, verify_all,
    verify_tag_sequence,
};
pub use encrypt::{DecryptedFile, EncryptedFile, decrypt, encrypt};
pub use error::{EncryptError, PasswordError};
pub use filename::{EXTENSION, FilenameError, decrypt_filename, encrypt_filename};
pub use password::{Key, prepare, validate_password};
