//! Deterministic padding in front of AES-FF1.
//!
//! The pad key is HKDF-SHA256 over `key.names`. The salt is
//! `naspresso-filename-pad-salt-v1`, the info is `naspresso-filename-pad-v1`,
//! and the output is 32 bytes.
//!
//! Symbols come from HMAC-SHA256 under that key. A block message is the domain
//! `naspresso-filename-pad-stream-v1`, the Base62 length as a big-endian `u32`,
//! the Base62 bytes, and the block counter as a big-endian `u32`. The counter
//! starts at 0 and increases by one per block.
//!
//! A stream byte is a Base62 index when it is below 248 (`byte % 62`). Any
//! larger byte is skipped. Filler symbols are taken first, then the marker.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use super::{base62, error::FilenameError};

const HKDF_SALT: &[u8] = b"naspresso-filename-pad-salt-v1";
const HKDF_INFO: &[u8] = b"naspresso-filename-pad-v1";
const HMAC_DOMAIN: &[u8] = b"naspresso-filename-pad-stream-v1";
/// `4 * 62`. Bytes at or above this are rejected so `% 62` is unbiased.
const REJECTION_BOUND: u8 = 248;

pub(crate) fn pad(names: &[u8; 32], encoded: &str) -> String {
    let len = encoded.len();
    let (filler, marker_mod) = if len.is_multiple_of(15) {
        (0, None)
    } else {
        let padding = 14 - (len % 15);
        (padding, Some(padding))
    };

    let mut stream = Stream::new(names, encoded);
    let mut out = String::with_capacity(len + filler + 1);
    out.push_str(encoded);
    for _ in 0..filler {
        out.push(base62::symbol(stream.next_index()).expect("base62 index"));
    }
    let marker = match marker_mod {
        None => stream.next_index(),
        Some(padding) => loop {
            let index = stream.next_index();
            if usize::from(index) % 14 == padding {
                break index;
            }
        },
    };
    out.push(base62::symbol(marker).expect("base62 index"));
    out
}

pub(crate) fn unpad(padded: &str) -> Result<&str, FilenameError> {
    let len = padded.len();
    if len < 15 || len % 15 > 1 {
        return Err(FilenameError::Length);
    }
    if len % 15 == 1 {
        return Ok(&padded[..len - 1]);
    }
    let marker = padded.as_bytes()[len - 1];
    let index = base62::index(marker).ok_or(FilenameError::Alphabet)?;
    let padding = usize::from(index) % 14;
    Ok(&padded[..len - 1 - padding])
}

fn pad_key(names: &[u8; 32]) -> [u8; 32] {
    let hkdf = hkdf::Hkdf::<Sha256>::new(Some(HKDF_SALT), names);
    let mut key = [0; 32];
    hkdf.expand(HKDF_INFO, &mut key).expect("hkdf expand");
    key
}

struct Stream {
    key: [u8; 32],
    prefix: Vec<u8>,
    counter: u32,
    block: [u8; 32],
    pos: usize,
}

impl Stream {
    fn new(names: &[u8; 32], encoded: &str) -> Self {
        let mut prefix = Vec::with_capacity(HMAC_DOMAIN.len() + 4 + encoded.len());
        prefix.extend_from_slice(HMAC_DOMAIN);
        prefix
            .extend_from_slice(&(u32::try_from(encoded.len()).expect("name length")).to_be_bytes());
        prefix.extend_from_slice(encoded.as_bytes());
        Self {
            key: pad_key(names),
            prefix,
            counter: 0,
            block: [0; 32],
            pos: 32,
        }
    }

    fn next_index(&mut self) -> u8 {
        loop {
            let byte = self.next_byte();
            if byte < REJECTION_BOUND {
                return byte % 62;
            }
        }
    }

    fn next_byte(&mut self) -> u8 {
        if self.pos == self.block.len() {
            let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("hmac key");
            mac.update(&self.prefix);
            mac.update(&self.counter.to_be_bytes());
            let tag = mac.finalize();
            self.block.copy_from_slice(tag.as_bytes());
            self.counter = self.counter.wrapping_add(1);
            self.pos = 0;
        }
        let byte = self.block[self.pos];
        self.pos += 1;
        byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded_len(len: usize) -> usize {
        if len.is_multiple_of(15) {
            len + 1
        } else {
            len + 15 - (len % 15)
        }
    }

    #[test]
    fn pads_the_length_table_and_removes_it() {
        let names = [0x11; 32];
        for len in [1, 2, 13, 14, 15, 16, 29, 30, 31, 44, 45, 46] {
            let encoded = format!("1{}", "0".repeat(len - 1));
            let padded = pad(&names, &encoded);
            assert_eq!(padded.len(), padded_len(len), "length {len}");
            assert_eq!(unpad(&padded).unwrap(), encoded);
            let marker = base62::index(*padded.as_bytes().last().unwrap()).unwrap();
            if len.is_multiple_of(15) {
                assert_eq!(padded.len() % 15, 1);
            } else {
                let padding = 14 - (len % 15);
                assert_eq!(usize::from(marker) % 14, padding);
            }
        }
    }

    #[test]
    fn padding_is_deterministic() {
        let names = [0x11; 32];
        assert_eq!(pad(&names, "1Z"), pad(&names, "1Z"));
    }
}
