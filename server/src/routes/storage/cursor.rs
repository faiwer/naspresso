//! Opaque `ls` page cursor: `[u8 sort tag][u8 kind][name as UTF-8]`, base64url
//! without padding. The tag pins a cursor to the order it was issued for.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::{
    error::AppError,
    storage::{EntryKey, EntryKind},
};

/// Directories first, then by name. The only order so far.
const SORT_BY_NAME: u8 = 0;

const KIND_DIR: u8 = 0;
const KIND_FILE: u8 = 1;

pub(super) fn encode_cursor(key: &EntryKey) -> String {
    let kind = match key.kind {
        EntryKind::Dir => KIND_DIR,
        EntryKind::File => KIND_FILE,
    };
    let mut buf = Vec::with_capacity(2 + key.name.len());
    buf.push(SORT_BY_NAME);
    buf.push(kind);
    buf.extend_from_slice(key.name.as_bytes());
    URL_SAFE_NO_PAD.encode(buf)
}

pub(super) fn decode_cursor(cursor: &str) -> Result<EntryKey, AppError> {
    let malformed = || AppError::BadRequest("malformed cursor".into());

    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| malformed())?;
    let [sort, kind, name @ ..] = bytes.as_slice() else {
        return Err(malformed());
    };
    if *sort != SORT_BY_NAME {
        return Err(AppError::BadRequest("cursor doesn't match sort".into()));
    }
    let kind = match *kind {
        KIND_DIR => EntryKind::Dir,
        KIND_FILE => EntryKind::File,
        _ => return Err(malformed()),
    };
    if name.is_empty() {
        return Err(malformed());
    }
    let name = std::str::from_utf8(name)
        .map_err(|_| malformed())?
        .to_owned();

    Ok(EntryKey { kind, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: EntryKind, name: &str) -> EntryKey {
        EntryKey {
            kind,
            name: name.to_owned(),
        }
    }

    fn is_bad_request(result: Result<EntryKey, AppError>) -> bool {
        matches!(result, Err(AppError::BadRequest(_)))
    }

    #[test]
    fn round_trips() {
        for key in [
            key(EntryKind::Dir, "photos"),
            key(EntryKind::File, "Отпуск 2026 (1).JPG"),
            key(EntryKind::File, "a+b/c?d=e"),
        ] {
            assert_eq!(decode_cursor(&encode_cursor(&key)).unwrap(), key);
        }
    }

    #[test]
    fn is_url_safe() {
        let cursor = encode_cursor(&key(EntryKind::File, "???>>>"));
        assert!(
            cursor
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "{cursor}"
        );
    }

    #[test]
    fn rejects_garbage() {
        let encode = |bytes: &[u8]| URL_SAFE_NO_PAD.encode(bytes);
        for cursor in [
            "not base64!".to_owned(),
            String::new(),
            encode(&[SORT_BY_NAME]),
            encode(&[SORT_BY_NAME, KIND_FILE]),
            encode(&[SORT_BY_NAME, 7, b'a']),
            encode(&[SORT_BY_NAME, KIND_FILE, 0xff]),
            encode(&[1, KIND_FILE, b'a']),
        ] {
            assert!(is_bad_request(decode_cursor(&cursor)), "{cursor:?}");
        }
    }
}
