use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use super::{
    error::ContentError,
    format::{EOF_DOMAIN, EOF_SIZE, HEADER_SIZE},
    keys::Keys,
};

pub(super) fn start(keys: &Keys, header: &[u8; HEADER_SIZE as usize]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&keys.eof).expect("eof hmac key");
    mac.update(EOF_DOMAIN);
    mac.update(header);
    mac
}

pub(super) fn finish(
    mac: Hmac<Sha256>,
    plain_len: u64,
    chunk_count: u64,
) -> [u8; EOF_SIZE as usize] {
    let mut mac = mac;
    mac.update(&plain_len.to_be_bytes());
    mac.update(&chunk_count.to_be_bytes());
    let tag = mac.finalize().into_bytes();
    let mut out = [0; EOF_SIZE as usize];
    out.copy_from_slice(&tag[..EOF_SIZE as usize]);
    out
}

pub(super) fn matches(
    expected: &[u8; EOF_SIZE as usize],
    actual: &[u8; EOF_SIZE as usize],
) -> Result<(), ContentError> {
    if expected.ct_eq(actual).unwrap_u8() == 1 {
        Ok(())
    } else {
        Err(ContentError::Authentication)
    }
}
