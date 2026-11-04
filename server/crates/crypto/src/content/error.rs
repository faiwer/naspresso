use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("reading encrypted content failed")]
    Io(#[from] io::Error),

    #[error("could not generate a nonce")]
    Random,

    #[error("encrypted content has the wrong magic")]
    Magic,

    #[error("encrypted content version is not supported")]
    Version,

    #[error("encrypted content has an invalid layout")]
    Layout,

    #[error("encrypted content failed authentication")]
    Authentication,
}
