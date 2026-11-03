use std::io;

use thiserror::Error;

use crate::filename::FilenameError;

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("writing the encrypted file failed")]
    Io(#[from] io::Error),

    #[error(transparent)]
    Filename(#[from] FilenameError),
}

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password must be 64 characters")]
    Length,

    #[error("password must be base62")]
    Symbol,

    #[error("could not derive the key")]
    Derive,
}
