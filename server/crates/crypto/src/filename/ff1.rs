use aes::Aes256;
use fpe::ff1::{FF1, FlexibleNumeralString};

use super::{base62, error::FilenameError};

const RADIX: u32 = 62;

pub(crate) fn encrypt(key: &[u8; 32], padded: &str) -> Result<String, FilenameError> {
    transform(key, padded, true)
}

pub(crate) fn decrypt(key: &[u8; 32], cipher: &str) -> Result<String, FilenameError> {
    transform(key, cipher, false)
}

fn transform(key: &[u8; 32], text: &str, encrypting: bool) -> Result<String, FilenameError> {
    let digits = numerals(text)?;
    let ff = FF1::<Aes256>::new(key, RADIX).expect("radix 62");
    let numerals = FlexibleNumeralString::from(digits);
    let out = if encrypting {
        ff.encrypt(&[], &numerals)
    } else {
        ff.decrypt(&[], &numerals)
    }
    .map_err(|_| FilenameError::Cipher)?;
    text_from(Vec::from(out))
}

fn numerals(text: &str) -> Result<Vec<u16>, FilenameError> {
    text.bytes()
        .map(|byte| {
            base62::index(byte)
                .map(u16::from)
                .ok_or(FilenameError::Alphabet)
        })
        .collect()
}

fn text_from(digits: Vec<u16>) -> Result<String, FilenameError> {
    digits
        .into_iter()
        .map(|digit| {
            let index = u8::try_from(digit).map_err(|_| FilenameError::Cipher)?;
            base62::symbol(index).ok_or(FilenameError::Cipher)
        })
        .collect()
}
