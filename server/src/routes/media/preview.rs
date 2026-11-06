use std::path::Path;

use axum::{
    body::Body,
    extract::{Path as UrlPath, State},
    http::HeaderMap,
    response::Response,
};
use media::preview::PreviewSize;
use tokio_util::io::ReaderStream;

use crate::{
    cancel::Cancel,
    error::AppError,
    preview::{OpenNpe, Preview},
    routes::{
        crypto::COOKIE_MAX_AGE,
        storage::{cannot_decrypt, vault_key},
        stream::{CacheControl, etag, not_modified, stream},
    },
    state::AppState,
    storage,
};

/// `GET /media/preview/256/a/b/c.jpg`: an AVIF preview of the image whose
/// longest side is at most 128, 256 or 512. Revalidates with `If-None-Match`.
/// An `.npe` file is decrypted with the vault cookie, and the browser may keep
/// that preview only as long as the cookie.
pub async fn preview(
    cancel: Cancel,
    State(state): State<AppState>,
    UrlPath((size, file)): UrlPath<(u32, String)>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let size = PreviewSize::try_from(size)?;
    let path = storage::resolve(&state.config.storage, &file).await?;
    let npe = open_if_npe(&file, &headers, state.config.npe_seal_key.as_bytes())?;
    let fingerprint = storage::open_file(&path).await?.fingerprint;
    let tag = etag(&fingerprint);
    let cache = cache_control(npe.is_some());
    if let Some(response) = not_modified(&headers, &tag, cache) {
        return Ok(response);
    }

    let preview = state
        .previews
        .get(&path, &fingerprint, size, &cancel, npe.as_ref())
        .await?;
    let (body, len) = match preview {
        Preview::Cached(avif) => (Body::from_stream(ReaderStream::new(avif.reader)), avif.len),
        Preview::Fresh(avif) => {
            let len = avif.len() as u64;
            (Body::from(avif), len)
        }
    };
    Ok(stream(body, len, "image/avif", tag, cache))
}

/// The vault key and decrypted name when `file` ends in `.npe`.
/// Any other file is `None`. A locked `.npe` is an error.
fn open_if_npe(
    file: &str,
    headers: &HeaderMap,
    secret: &[u8; 32],
) -> Result<Option<OpenNpe>, AppError> {
    let Some(name) = Path::new(file).file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if !name.ends_with(::crypto::EXTENSION) {
        return Ok(None);
    }
    let key = vault_key(file, headers, secret)?;
    let decrypted_name = ::crypto::decrypt_filename(&key, name).map_err(|_| cannot_decrypt())?;
    Ok(Some(OpenNpe {
        key,
        decrypted_name,
    }))
}

fn cache_control(npe: bool) -> CacheControl {
    if npe {
        CacheControl::Private {
            max_age: u64::try_from(COOKIE_MAX_AGE).unwrap_or(0),
        }
    } else {
        CacheControl::Revalidate
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::*;

    #[test]
    fn a_plain_file_needs_no_key() {
        assert!(
            open_if_npe("vault.nped/day.jpg", &HeaderMap::new(), &[7; 32])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_npe_without_a_cookie_is_refused() {
        assert!(matches!(
            open_if_npe("photos/note.npe", &HeaderMap::new(), &[7; 32]),
            Err(AppError::BadRequest(message)) if message == "cannot decrypt the file"
        ));
    }

    #[test]
    fn an_npe_cookie_yields_the_plain_name() {
        let secret = [9u8; 32];
        let key = ::crypto::Key {
            names: [3; 32],
            contents: [4; 32],
        };
        let disk = ::crypto::encrypt_filename(&key, "dot.png").unwrap();
        let file = format!("vault.nped/{disk}");
        let headers = cookie(&file, &secret, &key);

        let opened = open_if_npe(&file, &headers, &secret).unwrap().unwrap();
        assert_eq!(opened.decrypted_name, "dot.png");
        assert_eq!(opened.key.contents, key.contents);

        let wrong = format!("vault.nped/{disk}x.npe");
        assert!(matches!(
            open_if_npe(&wrong, &cookie(&file, &secret, &key), &secret),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn an_npe_preview_is_private_for_the_cookie_lifetime() {
        let tag = etag("0123456789abcdef");
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, tag.clone());

        let locked = not_modified(&headers, &tag, cache_control(true)).unwrap();
        assert_eq!(
            locked.headers().get(header::CACHE_CONTROL).unwrap(),
            &format!("private, max-age={COOKIE_MAX_AGE}, must-revalidate")
        );
        assert_eq!(locked.headers().get(header::VARY).unwrap(), "Cookie");

        let plain = not_modified(&headers, &tag, cache_control(false)).unwrap();
        assert_eq!(
            plain.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );
    }

    fn cookie(file: &str, secret: &[u8; 32], key: &::crypto::Key) -> HeaderMap {
        let vault = file.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(file);
        let name = crate::crypto::encode_cookie_name(vault);
        let value = crate::crypto::seal(secret, key).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{name}={value}")).unwrap(),
        );
        headers
    }
}
