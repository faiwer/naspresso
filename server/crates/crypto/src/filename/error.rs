use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FilenameError {
    #[error("filename is empty")]
    Empty,

    #[error("filename is not allowed")]
    Invalid,

    #[error("encrypted filename exceeds 255 bytes")]
    TooLong,

    #[error("encrypted filename is not base62")]
    Alphabet,

    #[error("encrypted filename has an invalid length")]
    Length,

    #[error("encrypted filename is not valid UTF-8")]
    Utf8,

    #[error("filename encryption failed")]
    Cipher,
}
