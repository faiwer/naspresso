use std::{io::SeekFrom, path::Path};

use axum::{
    body::Body,
    extract::{Path as UrlPath, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::Response,
};
use crypto::ContentError;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{cannot_decrypt, vault_key};
use crate::{
    error::AppError,
    routes::{
        crypto::COOKIE_MAX_AGE,
        stream::{CacheControl, etag, not_modified, stream},
    },
    state::AppState,
    storage::{self, SNIFF_LEN, sniff_mime},
};

#[derive(Deserialize)]
pub(super) struct DecodeQuery {
    /// Decrypt an `.npe` file with the vault cookie. Absent means the stored bytes.
    #[serde(default)]
    decode: bool,
}

/// `GET /storage/file/a/b/c.jpg`: the file's bytes, streamed. Revalidates
/// with `If-None-Match`; `Range` requests are refused. `?decode=true` returns
/// the plaintext of an unlocked `.npe` file.
pub async fn file(
    State(state): State<AppState>,
    UrlPath(file): UrlPath<String>,
    Query(query): Query<DecodeQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if headers.contains_key(header::RANGE) {
        return Err(AppError::RangeNotSupported);
    }

    let path = storage::resolve(&state.config.storage, &file).await?;
    if query.decode {
        return decoded_file(&state, &file, &path, &headers).await;
    }

    let mut opened = storage::open_file(&path).await?;
    let tag = etag(&opened.fingerprint);
    if let Some(response) = not_modified(&headers, &tag, CacheControl::Revalidate) {
        return Ok(response);
    }

    let mime = opened.mime().await?;
    let body = Body::from_stream(ReaderStream::new(opened.file));
    let mut response = stream(body, opened.size, mime, tag, CacheControl::Revalidate);
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        content_disposition(download_name(&path)?)?,
    );
    Ok(response)
}

async fn decoded_file(
    state: &AppState,
    file: &str,
    path: &Path,
    headers: &HeaderMap,
) -> Result<Response, AppError> {
    let key = vault_key(file, headers, state.config.npe_seal_key.as_bytes())?;
    let plain_name =
        ::crypto::decrypt_filename(&key, download_name(path)?).map_err(|_| cannot_decrypt())?;

    let opened = storage::open_file(path).await?;
    let tag = etag(&opened.fingerprint);
    let cache = CacheControl::Private {
        max_age: u64::try_from(COOKIE_MAX_AGE).unwrap_or(0),
    };
    if let Some(response) = not_modified(headers, &tag, cache) {
        return Ok(response);
    }

    let mut decoded = ::crypto::AsyncContentFile::open(&key, opened.file)
        .await
        .map_err(content_error)?;
    let plain_len = decoded.plaintext_len();
    let mime = plaintext_mime(&mut decoded, plain_len, &plain_name, path, &key).await?;
    let mut response = stream(
        Body::from_stream(ReaderStream::new(decoded)),
        plain_len,
        mime,
        tag,
        cache,
    );
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition(&plain_name)?,
    );
    Ok(response)
}

async fn plaintext_mime<R>(
    decoded: &mut ::crypto::AsyncContentFile<R>,
    plain_len: u64,
    plain_name: &str,
    path: &Path,
    key: &::crypto::Key,
) -> Result<&'static str, AppError>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin,
{
    if plain_len == 0 {
        authenticate_empty(path, key).await?;
        return Ok(sniff_mime(&[], 0, Path::new(plain_name)));
    }

    let n = usize::try_from(plain_len.min(SNIFF_LEN as u64)).unwrap_or(0);
    let mut head = vec![0; n];
    decoded.read_exact(&mut head).await.map_err(read_error)?;
    decoded.seek(SeekFrom::Start(0)).await.map_err(read_error)?;
    Ok(sniff_mime(&head, plain_len, Path::new(plain_name)))
}

async fn authenticate_empty(path: &Path, key: &::crypto::Key) -> Result<(), AppError> {
    let path = path.to_path_buf();
    let key = key.clone();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(path)?;
        ::crypto::verify_tag_sequence(&key, &mut file).map_err(content_io)
    })
    .await
    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    .map_err(read_error)
}

fn download_name(path: &Path) -> Result<&str, AppError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::BadRequest("file name is not valid".to_owned()))
}

/// `inline`, so a preview still displays. `filename*` is the name the save dialog uses.
fn content_disposition(name: &str) -> Result<HeaderValue, AppError> {
    let value = format!(
        "inline; filename=\"{}\"; filename*=UTF-8''{}",
        quoted_filename(name),
        percent_encode(name),
    );
    HeaderValue::from_str(&value).map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}

fn quoted_filename(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '"' | '\\' => '_',
            ch if ch.is_ascii() && !ch.is_control() => ch,
            _ => '_',
        })
        .collect()
}

fn percent_encode(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn content_error(error: ContentError) -> AppError {
    match error {
        ContentError::Io(error) => AppError::Internal(error.into()),
        _ => cannot_decrypt(),
    }
}

fn content_io(error: ContentError) -> std::io::Error {
    match error {
        ContentError::Io(error) => error,
        other => std::io::Error::new(std::io::ErrorKind::InvalidData, other),
    }
}

fn read_error(error: std::io::Error) -> AppError {
    if error.kind() == std::io::ErrorKind::InvalidData {
        cannot_decrypt()
    } else {
        AppError::Internal(error.into())
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, path::PathBuf};

    use tokio::io::AsyncReadExt;

    use super::*;

    #[test]
    fn save_dialog_uses_the_decrypted_name() {
        let header = content_disposition("café.gif").unwrap();
        let header = header.to_str().unwrap();
        assert!(header.contains("filename=\"caf_.gif\""));
        assert!(header.contains("filename*=UTF-8''caf%C3%A9.gif"));
    }

    #[tokio::test]
    async fn decode_reads_the_plaintext() {
        let tmp = Temp::new();
        let key = ::crypto::Key {
            names: [3; 32],
            contents: [4; 32],
        };
        let gif = b"GIF89a\x03\x00\x02\x00\x00\x00\x00;";
        let disk = ::crypto::encrypt_filename(&key, "tiny.gif").unwrap();
        let mut input = Cursor::new(gif.to_vec());
        let mut output = Cursor::new(Vec::new());
        ::crypto::encrypt_content(&key, &mut input, &mut output).unwrap();
        let path = tmp.0.join(&disk);
        std::fs::write(&path, output.into_inner()).unwrap();

        let file = tokio::fs::File::open(&path).await.unwrap();
        let mut decoded = ::crypto::AsyncContentFile::open(&key, file).await.unwrap();
        let len = decoded.plaintext_len();
        let mime = plaintext_mime(&mut decoded, len, "tiny.gif", &path, &key)
            .await
            .unwrap();
        assert_eq!(mime, "image/gif");
        let mut body = Vec::new();
        decoded.read_to_end(&mut body).await.unwrap();
        assert_eq!(body, gif);

        let wrong = ::crypto::Key {
            names: key.names,
            contents: [9; 32],
        };
        let file = tokio::fs::File::open(&path).await.unwrap();
        let mut decoded = ::crypto::AsyncContentFile::open(&wrong, file)
            .await
            .unwrap();
        let len = decoded.plaintext_len();
        let error = plaintext_mime(&mut decoded, len, "tiny.gif", &path, &wrong)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("npe-file-{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
