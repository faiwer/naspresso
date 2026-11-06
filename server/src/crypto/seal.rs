use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use chrono::{DateTime, Utc};

use super::CryptoError;

const NONCE_LEN: usize = 24;
const DIGEST: usize = 32;
const EXPIRY_LEN: usize = 8;
const PLAIN_LEN: usize = DIGEST * 2 + EXPIRY_LEN;
const TAG_LEN: usize = 16;

/// A vault key and the instant its unlock cookie locks again.
pub struct SealedKey {
    pub key: ::crypto::Key,
    pub expired_at: DateTime<Utc>,
}

/// Encrypt `key` and `expired_at` with the server seal key. The result is
/// unpadded base64url of `nonce || ciphertext || tag`.
pub fn seal(
    secret: &[u8; 32],
    key: &::crypto::Key,
    expired_at: DateTime<Utc>,
) -> Result<String, CryptoError> {
    let cipher = cipher(secret)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|_| CryptoError::Seal)?;
    let nonce = XNonce::from(nonce_bytes);
    let plain = plain(key, expired_at);
    let cipher_text = cipher
        .encrypt(&nonce, plain.as_slice())
        .map_err(|_| CryptoError::Seal)?;
    let mut packed = Vec::with_capacity(NONCE_LEN + cipher_text.len());
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&cipher_text);
    Ok(URL_SAFE_NO_PAD.encode(packed))
}

fn cipher(secret: &[u8; 32]) -> Result<XChaCha20Poly1305, CryptoError> {
    XChaCha20Poly1305::new_from_slice(secret).map_err(|_| CryptoError::Seal)
}

fn plain(key: &::crypto::Key, expired_at: DateTime<Utc>) -> [u8; PLAIN_LEN] {
    let mut plain = [0u8; PLAIN_LEN];
    plain[..DIGEST].copy_from_slice(&key.names);
    plain[DIGEST..DIGEST * 2].copy_from_slice(&key.contents);
    plain[DIGEST * 2..].copy_from_slice(&expired_at.timestamp_millis().to_be_bytes());
    plain
}

/// Inverse of [`seal`]. A cookie sealed before the expiry was stored does not open.
pub fn unseal(secret: &[u8; 32], value: &str) -> Result<SealedKey, CryptoError> {
    let packed = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| CryptoError::Seal)?;
    if packed.len() != NONCE_LEN + PLAIN_LEN + TAG_LEN {
        return Err(CryptoError::Seal);
    }

    let (nonce_bytes, cipher_text) = packed.split_at(NONCE_LEN);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(nonce_bytes);

    let plain = cipher(secret)?
        .decrypt(&XNonce::from(nonce), cipher_text)
        .map_err(|_| CryptoError::Seal)?;
    if plain.len() != PLAIN_LEN {
        return Err(CryptoError::Seal);
    }

    let mut names = [0u8; DIGEST];
    let mut contents = [0u8; DIGEST];
    names.copy_from_slice(&plain[..DIGEST]);
    contents.copy_from_slice(&plain[DIGEST..DIGEST * 2]);
    let mut millis = [0u8; EXPIRY_LEN];
    millis.copy_from_slice(&plain[DIGEST * 2..]);
    let expired_at =
        DateTime::from_timestamp_millis(i64::from_be_bytes(millis)).ok_or(CryptoError::Seal)?;
    Ok(SealedKey {
        key: ::crypto::Key { names, contents },
        expired_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8) -> ::crypto::Key {
        ::crypto::Key {
            names: [byte; DIGEST],
            contents: [byte.wrapping_add(1); DIGEST],
        }
    }

    #[test]
    fn round_trips_a_key() {
        let secret = [7u8; 32];
        let key = sample(3);
        let expired_at = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let sealed = seal(&secret, &key, expired_at).unwrap();
        let opened = unseal(&secret, &sealed).unwrap();
        assert_eq!(opened.key.names, key.names);
        assert_eq!(opened.key.contents, key.contents);
        assert_eq!(opened.expired_at, expired_at);
        assert!(!sealed.contains('='));
    }

    #[test]
    fn rejects_a_tampered_value_and_the_wrong_secret() {
        let secret = [7u8; 32];
        let expired_at = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let sealed = seal(&secret, &sample(3), expired_at).unwrap();
        let mut bytes = URL_SAFE_NO_PAD.decode(&sealed).unwrap();
        bytes[NONCE_LEN] ^= 1;
        let tampered = URL_SAFE_NO_PAD.encode(bytes);
        assert!(unseal(&secret, &tampered).is_err());
        assert!(unseal(&[8u8; 32], &sealed).is_err());
    }
}
