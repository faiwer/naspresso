//! `key.contents` splits into an AEAD key and an EOF HMAC key. `key.names` is not an input.

use hkdf::Hkdf;
use sha2::Sha256;

use super::format::{AEAD_INFO, EOF_INFO, HKDF_SALT};

pub(super) struct Keys {
    pub aead: [u8; 32],
    pub eof: [u8; 32],
}

pub(super) fn derive(contents: &[u8; 32]) -> Keys {
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), contents);
    let mut aead = [0; 32];
    let mut eof = [0; 32];
    hkdf.expand(AEAD_INFO, &mut aead).expect("aead key length");
    hkdf.expand(EOF_INFO, &mut eof).expect("eof key length");
    Keys { aead, eof }
}
