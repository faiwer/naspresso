//! Unlocking an encrypted directory. Knows nothing about HTTP.

mod cookie;
mod error;
mod probe;
mod seal;

pub use cookie::encode_cookie_name;
pub use error::CryptoError;
pub use probe::{Probe, ensure_encrypted_dir, find_encrypted_file, probe};
pub use seal::{SealedKey, seal, unseal};
