use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, header},
};
use serde::{Deserialize, Serialize};

use super::cursor::{decode_cursor, encode_cursor};
use crate::{
    crypto,
    error::AppError,
    routes::cookie::cookie_value,
    state::AppState,
    storage::{self, Encryption, Entry, EntryKey},
};

const DEFAULT_LIMIT: usize = 30;
const MAX_LIMIT: usize = 100;

#[derive(Deserialize)]
pub struct LsQuery {
    /// Directory inside `STORAGE`, e.g. `/a/b/c`. Defaults to the root.
    #[serde(default)]
    path: String,
    /// Page size, `1..=MAX_LIMIT`. Defaults to `DEFAULT_LIMIT`.
    limit: Option<usize>,
    /// `next_cursor` of the previous page; omitted for the first one.
    cursor: Option<String>,
}

#[derive(Serialize)]
pub struct LsResponse {
    items: Vec<Entry>,
    /// Pass back verbatim as `?cursor=` for the next page; `null` on the last one.
    next_cursor: Option<String>,
    /// Entries in the whole directory, across all pages.
    total: usize,
    /// `none` when `path` never enters an `.nped` directory. Otherwise `locked`
    /// or `unlocked`, from that directory's cookie.
    encryption: Encryption,
}

/// `GET /storage/ls?path=/a/b/c&limit=30&cursor=…`: a page of the directory's
/// entries, directories first, then by name ignoring case.
pub async fn ls(
    State(state): State<AppState>,
    Query(query): Query<LsQuery>,
    headers: HeaderMap,
) -> Result<Json<LsResponse>, AppError> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;

    let dir = storage::resolve(&state.config.storage, &query.path).await?;
    let (encryption, key) = open_vault(
        &query.path,
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
        state.config.npe_seal_key.as_bytes(),
    );
    let previews = state.previews.clone();
    let page = storage::list_dir(
        &dir,
        after,
        limit,
        encryption,
        key,
        move |path, modified| previews.is_fresh(path, modified),
    )
    .await?;
    let next_cursor = match page.entries.last() {
        Some(last) if page.has_more => Some(encode_cursor(&EntryKey::of(last))),
        _ => None,
    };

    let pending = page.pending;
    if !pending.is_empty() {
        // Detached on purpose: dropping the handle does not cancel the write.
        std::mem::drop(tokio::task::spawn_blocking(move || {
            storage::write_records(pending);
        }));
    }

    Ok(Json(LsResponse {
        items: page.entries,
        next_cursor,
        total: page.total,
        encryption: page.encryption,
    }))
}

/// The innermost `.nped` directory on `path`. Its cookie unlocks the names inside.
fn open_vault(
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
