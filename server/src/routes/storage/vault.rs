use crate::{crypto, routes::cookie::cookie_value, storage::Encryption};

/// The innermost `.nped` directory on `path`, and the key its cookie unseals.
pub(super) fn open_vault(
    path: &str,
    cookie: Option<&str>,
    secret: &[u8; 32],
) -> (Encryption, Option<::crypto::Key>) {
    let Some(vault) = encrypted_dir(path) else {
        return (Encryption::None, None);
    };
    let name = crypto::encode_cookie_name(&vault);
    let Some(value) = cookie.and_then(|header| cookie_value(header, &name)) else {
        return (Encryption::Locked, None);
    };
    match crypto::unseal(secret, value) {
        Ok(key) => (Encryption::Unlocked, Some(key)),
        Err(_) => (Encryption::Locked, None),
    }
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
        assert_eq!(open_vault("", None, &secret).0, Encryption::None);
        assert_eq!(open_vault("/photos/day", None, &secret).0, Encryption::None);
    }

    #[test]
    fn the_innermost_dir_is_locked_until_its_cookie_opens() {
        let path = "/a.nped/b.nped/album";
        let secret = [7u8; 32];
        let key = ::crypto::Key {
            names: [1; 32],
            contents: [2; 32],
        };
        assert_eq!(open_vault(path, None, &secret).0, Encryption::Locked);

        let outer = crypto::encode_cookie_name("/a.nped");
        let sealed = crypto::seal(&secret, &key).unwrap();
        assert_eq!(
            open_vault(path, Some(&format!("{outer}={sealed}")), &secret).0,
            Encryption::Locked
        );
        assert_eq!(
            open_vault(path, Some(&format!("{outer}=sealed")), &secret).0,
            Encryption::Locked
        );

        let inner = crypto::encode_cookie_name("/a.nped/b.nped");
        let header = format!("other=1; {inner}={sealed}");
        let (encryption, opened) = open_vault(path, Some(&header), &secret);
        assert_eq!(encryption, Encryption::Unlocked);
        assert_eq!(opened.unwrap().names, key.names);
    }
}
