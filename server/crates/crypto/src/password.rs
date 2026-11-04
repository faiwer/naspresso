use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::PasswordError;

const LEN: usize = 64;
const DIGEST: usize = 32;
/// Argon2id memory cost, in KiB. Two iterations, one lane.
const MEMORY_KIB: u32 = 19_456;
const ITERATIONS: u32 = 20;
const LANES: u32 = 1;
const NAMES_SALT: &[u8] = b"naspresso-names-v1";
const CONTENTS_SALT: &[u8] = b"naspresso-contents-v1";

/// Argon2id digests of the password, split for file names and file contents.
#[derive(PartialEq, Eq)]
pub struct Key {
    pub names: [u8; DIGEST],
    pub contents: [u8; DIGEST],
}

/// Hash the password into a [`Key`].
///
/// `names` is Argon2id of the first 32 characters with salt `naspresso-names-v1`.
/// `contents` is Argon2id of the whole password with salt `naspresso-contents-v1`.
pub fn prepare(password: &str) -> Result<Key, PasswordError> {
    validate_password(password)?;
    let bytes = password.as_bytes();
    Ok(Key {
        names: digest(NAMES_SALT, &bytes[0..32])?,
        contents: digest(CONTENTS_SALT, bytes)?,
    })
}

/// Accept a password of exactly 64 base62 characters (`0-9`, `A-Z`, `a-z`).
pub fn validate_password(password: &str) -> Result<(), PasswordError> {
    if password.chars().count() != LEN {
        return Err(PasswordError::Length);
    }
    if !password.bytes().all(is_base62) {
        return Err(PasswordError::Symbol);
    }
    Ok(())
}

fn is_base62(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}

fn digest(salt: &[u8], input: &[u8]) -> Result<[u8; DIGEST], PasswordError> {
    let params =
        Params::new(MEMORY_KIB, ITERATIONS, LANES, Some(DIGEST)).expect("argon2 parameters");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0; DIGEST];
    argon
        .hash_password_into(input, salt, &mut out)
        .map_err(|_| PasswordError::Derive)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(last: u8) -> String {
        let mut password = "a".repeat(LEN);
        password.replace_range(LEN - 1..LEN, std::str::from_utf8(&[last]).unwrap());
        password
    }

    #[test]
    fn accepts_64_base62_characters() {
        let password = "a".repeat(32) + &"Z".repeat(16) + &"9".repeat(16);
        assert!(validate_password(&password).is_ok());
    }

    #[test]
    fn rejects_the_wrong_length() {
        assert!(matches!(
            validate_password(&"a".repeat(63)),
            Err(PasswordError::Length)
        ));
        assert!(matches!(
            validate_password(&"a".repeat(65)),
            Err(PasswordError::Length)
        ));
    }

    #[test]
    fn derives_two_different_stable_keys() {
        let password = "a".repeat(LEN);
        let first = prepare(&password).unwrap();
        let second = prepare(&password).unwrap();
        assert_eq!(first.names, second.names);
        assert_eq!(first.contents, second.contents);
        assert_ne!(first.names, first.contents);
    }

    #[test]
    fn rejects_symbols_outside_base62() {
        for symbol in *b"+/ _." {
            assert!(
                matches!(
                    validate_password(&sample(symbol)),
                    Err(PasswordError::Symbol)
                ),
                "{}",
                symbol as char
            );
        }
    }
}
