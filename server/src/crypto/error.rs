use crypto::ContentError;
use thiserror::Error;

use crate::storage::StorageError;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("not an encrypted directory")]
    NotEncryptedDirectory,

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Content(ContentError),

    #[error("could not seal the key")]
    Seal,
}
