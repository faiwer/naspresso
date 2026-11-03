use crate::error::PasswordError;

const LEN: usize = 64;

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
