//! Authenticated, seekable file contents. Filename encryption is a separate module.

mod async_file;
mod chunk;
mod error;
mod format;
mod keys;
mod mac; // message authentication code
mod read;
mod verify;
mod write;

pub use async_file::AsyncContentFile;
pub use error::ContentError;
pub use read::{ContentFile, decrypt as decrypt_content};
pub use verify::{verify_all, verify_tag_sequence};
pub use write::encrypt as encrypt_content;

#[cfg(test)]
mod tests;
