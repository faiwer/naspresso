use std::io;

use thiserror::Error;

use crate::{content::ContentError, filename::FilenameError};

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("writing the file failed")]
    Io(#[from] io::Error),

    #[error(transparent)]
    Filename(#[from] FilenameError),

    #[error(transparent)]
    Content(ContentError),
}

impl From<ContentError> for EncryptError {
    fn from(error: ContentError) -> Self {
        match error {
            ContentError::Io(error) => EncryptError::Io(error),
            other => EncryptError::Content(other),
        }
    }
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
