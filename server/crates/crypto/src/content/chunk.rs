use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{AeadInOut, KeyInit, array::Array, inout::InOutBuf},
};

use super::{
    error::ContentError,
    format::{HEADER_SIZE, aad, nonce},
    keys::Keys,
};

pub(super) fn seal(
    keys: &Keys,
    header: &[u8; HEADER_SIZE as usize],
    index: u64,
    plain: &mut [u8],
) -> Result<[u8; 16], ContentError> {
    let cipher = XChaCha20Poly1305::new((&keys.aead).into());
    let nonce = XNonce::from(nonce(header, index));
    let tag = cipher
        .encrypt_inout_detached(&nonce, &aad(header, index), InOutBuf::from(plain))
        .map_err(|_| ContentError::Authentication)?;
    Ok(tag_bytes(&tag))
}

pub(super) fn open(
    keys: &Keys,
    header: &[u8; HEADER_SIZE as usize],
    index: u64,
    cipher_text: &mut [u8],
    tag: &[u8; 16],
) -> Result<(), ContentError> {
    let cipher = XChaCha20Poly1305::new((&keys.aead).into());
    let nonce = XNonce::from(nonce(header, index));
    cipher
        .decrypt_inout_detached(
            &nonce,
            &aad(header, index),
            InOutBuf::from(cipher_text),
            &Array::from(*tag),
        )
        .map_err(|_| ContentError::Authentication)
}

fn tag_bytes(tag: &Array<u8, chacha20poly1305::aead::consts::U16>) -> [u8; 16] {
    let mut out = [0; 16];
    out.copy_from_slice(tag.as_slice());
    out
}
