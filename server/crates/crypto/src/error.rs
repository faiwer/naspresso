use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("writing the encrypted file failed")]
    Io(#[from] io::Error),
}
