use axum::http::{HeaderMap, header};
use chrono::{DateTime, Utc};

use crate::{
    crypto,
    error::{AppError, ClientErrorCode},
    routes::cookie::cookie_value,
    storage::Encryption,
};

pub(super) struct OpenedVault {
    pub encryption: Encryption,
    pub key: Option<::crypto::Key>,
    /// When `encryption` is [`Encryption::Unlocked`], the instant it locks again.
    pub lock_at: Option<DateTime<Utc>>,
}

/// The innermost `.nped` directory on `path`, and the key its cookie unseals.
pub(super) fn open_vault(path: &str, cookie: Option<&str>, secret: &[u8; 32]) -> OpenedVault {
    let Some(vault) = encrypted_dir(path) else {
        return closed(Encryption::None);
    };
    let name = crypto::encode_cookie_name(&vault);
    let Some(value) = cookie.and_then(|header| cookie_value(header, &name)) else {
        return closed(Encryption::Locked);
    };
    match crypto::unseal(secret, value) {
        Ok(crypto::SealedKey { key, expired_at }) => OpenedVault {
            encryption: Encryption::Unlocked,
            key: Some(key),
            lock_at: Some(expired_at),
        },
        Err(_) => closed(Encryption::Locked),
    }
}

fn closed(encryption: Encryption) -> OpenedVault {
    OpenedVault {
        encryption,
        key: None,
        lock_at: None,
    }
}

/// The vault key for `file`, from its cookie. Missing or unusable is the same
/// refusal a locked download gets.
pub(crate) fn vault_key(
    file: &str,
    headers: &HeaderMap,
    secret: &[u8; 32],
) -> Result<::crypto::Key, AppError> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    open_vault(file, cookie, secret)
        .key
        .ok_or_else(cannot_decrypt)
}

pub(crate) fn cannot_decrypt() -> AppError {
    AppError::bad_request(ClientErrorCode::CannotDecrypt, "cannot decrypt the file")
}

/// `/a/vault.nped/b` → `/a/vault.nped`. The innermost one, when several are on the path.
fn encrypted_dir(path: &str) -> Option<String> {
    let mut vault = None;
    let mut acc = String::new();
    for segment in path.trim_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(segment);
        if segment.ends_with(".nped") {
            vault = Some(format!("/{acc}"));
        }
    }
    vault
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_outside_an_encrypted_dir_is_none() {
        let secret = [7u8; 32];
        assert_eq!(open_vault("", None, &secret).encryption, Encryption::None);
        assert_eq!(
            open_vault("/photos/day", None, &secret).encryption,
            Encryption::None
        );
    }

    #[test]
    fn the_innermost_dir_is_locked_until_its_cookie_opens() {
        let path = "/a.nped/b.nped/album";
        let secret = [7u8; 32];
        let key = ::crypto::Key {
            names: [1; 32],
            contents: [2; 32],
        };
        assert_eq!(
            open_vault(path, None, &secret).encryption,
            Encryption::Locked
        );

        let outer = crypto::encode_cookie_name("/a.nped");
        let expired_at = chrono::DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let sealed = crypto::seal(&secret, &key, expired_at).unwrap();
        assert_eq!(
            open_vault(path, Some(&format!("{outer}={sealed}")), &secret).encryption,
            Encryption::Locked
        );
        assert_eq!(
            open_vault(path, Some(&format!("{outer}=sealed")), &secret).encryption,
            Encryption::Locked
        );

        let inner = crypto::encode_cookie_name("/a.nped/b.nped");
        let header = format!("other=1; {inner}={sealed}");
        let opened = open_vault(path, Some(&header), &secret);
        assert_eq!(opened.encryption, Encryption::Unlocked);
        assert_eq!(opened.key.unwrap().names, key.names);
        assert_eq!(opened.lock_at, Some(expired_at));
    }
}
