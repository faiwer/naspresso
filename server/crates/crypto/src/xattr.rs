//! FF1-AES-256 over raw bytes for a fixed metadata record.
//!
//! The key is HKDF-SHA256 over `key.names`. The salt is
//! `naspresso-xattr-salt-v1`, the info is `naspresso-xattr-ff1-v1`, and the
//! output is 32 bytes. The filename cipher keeps the raw `names` key.

use aes::Aes256;
use fpe::ff1::{FF1, FlexibleNumeralString};
use sha2::Sha256;

const RADIX: u32 = 256;
const HKDF_SALT: &[u8] = b"naspresso-xattr-salt-v1";
const HKDF_INFO: &[u8] = b"naspresso-xattr-ff1-v1";

/// Encrypt `plain` under `names`. `tweak` is the decoded leaf name. The
/// ciphertext has the same length as `plain`.
pub fn seal_xattr(names: &[u8; 32], tweak: &[u8], plain: &[u8]) -> Vec<u8> {
    transform(names, tweak, plain, true)
}

/// Invert [`seal_xattr`].
pub fn open_xattr(names: &[u8; 32], tweak: &[u8], cipher: &[u8]) -> Vec<u8> {
    transform(names, tweak, cipher, false)
}

fn transform(names: &[u8; 32], tweak: &[u8], bytes: &[u8], encrypting: bool) -> Vec<u8> {
    let ff = FF1::<Aes256>::new(&ff1_key(names), RADIX).expect("radix 256");
    let numerals = FlexibleNumeralString::from(digits(bytes));
    let out = if encrypting {
        ff.encrypt(tweak, &numerals)
    } else {
        ff.decrypt(tweak, &numerals)
    }
    .expect("xattr length");
    undigits(Vec::from(out))
}

fn ff1_key(names: &[u8; 32]) -> [u8; 32] {
    let hkdf = hkdf::Hkdf::<Sha256>::new(Some(HKDF_SALT), names);
    let mut key = [0; 32];
    hkdf.expand(HKDF_INFO, &mut key).expect("hkdf expand");
    key
}

fn digits(bytes: &[u8]) -> Vec<u16> {
    bytes.iter().map(|byte| u16::from(*byte)).collect()
}

fn undigits(digits: Vec<u16>) -> Vec<u8> {
    digits
        .into_iter()
        .map(|digit| u8::try_from(digit).expect("radix 256 digit"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_keeps_the_length() {
        let names = [0x11; 32];
        let plain = b"image/jpeg\0pad-to-fixed-record!!";
        let cipher = seal_xattr(&names, b"photo.jpg", plain);
        assert_eq!(cipher.len(), plain.len());
        assert_ne!(cipher, plain);
        assert_eq!(open_xattr(&names, b"photo.jpg", &cipher), plain);
    }

    #[test]
    fn the_tweak_and_the_key_both_change_the_ciphertext() {
        let names = [0x11; 32];
        let other = [0x22; 32];
        let plain = [0x5a; 48];
        let cipher = seal_xattr(&names, b"photo.jpg", &plain);
        assert_ne!(seal_xattr(&names, b"song.mp3", &plain), cipher);
        assert_ne!(seal_xattr(&other, b"photo.jpg", &plain), cipher);
        assert_ne!(open_xattr(&names, b"song.mp3", &cipher), plain);
        assert_ne!(open_xattr(&other, b"photo.jpg", &cipher), plain);
    }
}
