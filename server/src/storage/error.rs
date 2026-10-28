use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    /// The client-supplied path has an empty, `.` or `..` segment, or a forbidden character.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("not found")]
    NotFound,

    #[error("not a directory")]
    NotADirectory,

    #[error(transparent)]
    Io(#[from] io::Error),
}
