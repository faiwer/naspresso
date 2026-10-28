use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::{error::AppError, state::AppState, storage};

#[derive(Deserialize)]
pub struct FileQuery {
    /// File inside `STORAGE`, e.g. `/a/b/c.jpg`.
    file: String,
}

/// `GET /storage/file?file=/a/b/c.jpg`: the file's bytes, streamed. Revalidates
/// with `If-None-Match`; `Range` requests are refused.
pub async fn file(
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if headers.contains_key(header::RANGE) {
        return Err(AppError::RangeNotSupported);
    }

    let path = storage::resolve(&state.config.storage, &query.file).await?;
    let mut opened = storage::open_file(&path).await?;
    let etag = header_value(format!("\"{}\"", opened.fingerprint));

    if headers.get(header::IF_NONE_MATCH) == Some(&etag) {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        set_cache_headers(response.headers_mut(), etag);
        return Ok(response);
    }

    let mime = opened.mime().await?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(opened.file)));
    let response_headers = response.headers_mut();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(opened.size));
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
    set_cache_headers(response_headers, etag);
    Ok(response)
}

fn set_cache_headers(headers: &mut HeaderMap, etag: HeaderValue) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ETAG, etag);
}

fn header_value(value: String) -> HeaderValue {
    HeaderValue::try_from(value).expect("a hex etag is a valid header value")
}
