use crate::Key;

use super::{base62, error::FilenameError, ff1, pad};

/// Longest file name on macOS and Linux, in bytes. 255 itself is allowed.
const MAX_COMPONENT: usize = 255;
/// File extension of an encrypted file. It is not part of the FF1 ciphertext.
pub const EXTENSION: &str = ".npe";

/// Encrypt `file_name` with `key.names`. The result ends with [`EXTENSION`].
pub fn encrypt_filename(key: &Key, file_name: &str) -> Result<String, FilenameError> {
    check_name(file_name)?;
    let encoded = base62::encode(file_name.as_bytes());
    let padded = pad::pad(&key.names, &encoded);
    if padded.len() + EXTENSION.len() > MAX_COMPONENT {
        return Err(FilenameError::TooLong);
    }
    // ff1 does not change the length.
    let cipher = ff1::encrypt(&key.names, &padded)?;
    Ok(format!("{cipher}{EXTENSION}"))
}

/// Invert [`encrypt_filename`]. `encrypted_name` ends with [`EXTENSION`].
pub fn decrypt_filename(key: &Key, encrypted_name: &str) -> Result<String, FilenameError> {
    let Some(encrypted_name) = encrypted_name.strip_suffix(EXTENSION) else {
        return Err(FilenameError::Invalid);
    };
    if encrypted_name.is_empty() {
        return Err(FilenameError::Length);
    }
    if encrypted_name
        .bytes()
        .any(|byte| base62::index(byte).is_none())
    {
        return Err(FilenameError::Alphabet);
    }
    let len = encrypted_name.len();
    if len < 15 || len % 15 > 1 {
        return Err(FilenameError::Length);
    }
    let padded = ff1::decrypt(&key.names, encrypted_name)?;
    let encoded = pad::unpad(&padded)?;
    let bytes = base62::decode(encoded)?;
    String::from_utf8(bytes).map_err(|_| FilenameError::Utf8)
}

fn check_name(name: &str) -> Result<(), FilenameError> {
    if name.is_empty() {
        return Err(FilenameError::Empty);
    }
    if name == "." || name == ".." || name.bytes().any(|byte| matches!(byte, b'/' | b'\\' | 0)) {
        return Err(FilenameError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        super::{base62, ff1, pad},
        *,
    };

    fn key(byte: u8) -> Key {
        Key {
            names: [byte; 32],
            contents: [0; 32],
        }
    }

    const ROUND_TRIPS: [&str; 8] = [
        "a",
        "1.txt",
        "hello world.txt",
        "你好世界.txt",
        "😀🚀🔥.txt",
        "é.txt",
        "e\u{301}.txt",
        "name with spaces.txt",
    ];

    // `names` is 32 bytes of `0x11`. `contents` does not affect the result.
    #[test]
    fn matches_fixed_vectors() {
        let key = key(0x11);
        let vectors = [
            ("a", "uFOXOyXVLX5JPVl.npe"),
            ("1.txt", "2PyHvgTq0RHN4o2.npe"),
            ("hello world.txt", "P6x76BIQtBb7ZjecKRYsrrAJ1Z7kRP.npe"),
            ("你好世界.txt", "OBjHylYoGh87n9rGf6phzdxwoB8lK0.npe"),
            ("😀🚀🔥.txt", "0PDAteiqKVi8Unqr4d5BgUvYWzewDH.npe"),
            ("é.txt", "cfQi3ofwbx5rbUG.npe"),
            ("e\u{301}.txt", "luZCOJSL3nZ5jX6.npe"),
            ("name with spaces.txt", "w78bXaHnjdVwtHdODtYx7Lnp1TpTcl.npe"),
        ];
        for (name, cipher) in vectors {
            assert_eq!(encrypt_filename(&key, name).unwrap(), cipher);
            assert_eq!(decrypt_filename(&key, cipher).unwrap(), name);
        }
    }

    #[test]
    fn round_trips_sample_names() {
        let key = key(0x11);
        for name in ROUND_TRIPS {
            let cipher = encrypt_filename(&key, name).unwrap();
            assert_eq!(decrypt_filename(&key, &cipher).unwrap(), name);
            assert!(cipher.len() <= MAX_COMPONENT);
            let stem = cipher.strip_suffix(EXTENSION).unwrap();
            assert!(stem.bytes().all(|byte| base62::index(byte).is_some()));
        }
    }

    #[test]
    fn preserves_normalization_forms() {
        let key = key(0x11);
        let nfc = "é.txt";
        let nfd = "e\u{301}.txt";
        assert_ne!(nfc, nfd);
        assert_ne!(
            encrypt_filename(&key, nfc).unwrap(),
            encrypt_filename(&key, nfd).unwrap()
        );
        assert_eq!(
            decrypt_filename(&key, &encrypt_filename(&key, nfd).unwrap()).unwrap(),
            nfd
        );
    }

    #[test]
    fn is_deterministic_and_key_specific() {
        let name = "hello world.txt";
        let first = key(0x11);
        let mut same_contents = key(0x11);
        same_contents.contents = [0xab; 32];
        let other = key(0x22);
        let cipher = encrypt_filename(&first, name).unwrap();
        assert_eq!(encrypt_filename(&first, name).unwrap(), cipher);
        assert_eq!(encrypt_filename(&same_contents, name).unwrap(), cipher);
        assert_ne!(encrypt_filename(&other, name).unwrap(), cipher);
    }

    #[test]
    fn rejects_names_that_cannot_be_files() {
        let key = key(0x11);
        assert_eq!(encrypt_filename(&key, ""), Err(FilenameError::Empty));
        for name in [".", "..", "a/b", "a\\b", "a\0b"] {
            assert_eq!(encrypt_filename(&key, name), Err(FilenameError::Invalid));
        }
    }

    #[test]
    fn rejects_bad_ciphertext() {
        let key = key(0x11);
        assert_eq!(
            decrypt_filename(&key, "not base62!"),
            Err(FilenameError::Invalid)
        );
        assert_eq!(
            decrypt_filename(&key, &format!("{}{EXTENSION}", "a".repeat(14))),
            Err(FilenameError::Length)
        );
        assert_eq!(
            decrypt_filename(&key, &format!("{}{EXTENSION}", "a".repeat(17))),
            Err(FilenameError::Length)
        );
        assert_eq!(
            decrypt_filename(&key, &format!("not-base62{EXTENSION}")),
            Err(FilenameError::Alphabet)
        );
    }

    #[test]
    fn rejects_a_name_that_will_not_fit() {
        let key = key(0x11);
        let mut fit: Option<String> = None;
        for size in 1..400 {
            let name = "a".repeat(size);
            let encoded = base62::encode(name.as_bytes()).len();
            let padded = if encoded.is_multiple_of(15) {
                encoded + 1
            } else {
                encoded + 15 - (encoded % 15)
            };
            if padded + EXTENSION.len() > MAX_COMPONENT {
                assert_eq!(encrypt_filename(&key, &name), Err(FilenameError::TooLong));
                let shorter = fit.expect("a shorter name fits");
                assert!(encrypt_filename(&key, &shorter).is_ok());
                return;
            }
            fit = Some(name);
        }
        panic!("no filename exceeded 255 bytes");
    }

    #[test]
    fn round_trips_boundary_base62_lengths() {
        let names = [0x11; 32];
        for len in [1, 2, 13, 14, 15, 16, 29, 30, 31, 44, 45, 46] {
            let encoded = format!("1{}", "0".repeat(len - 1));
            let padded = pad::pad(&names, &encoded);
            let cipher = ff1::encrypt(&names, &padded).unwrap();
            assert_eq!(cipher.len(), padded.len());
            assert_eq!(ff1::decrypt(&names, &cipher).unwrap(), padded);
            assert_eq!(
                pad::unpad(&ff1::decrypt(&names, &cipher).unwrap()).unwrap(),
                encoded
            );
        }
    }
}
