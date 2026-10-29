use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::Response,
};
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::{
    error::AppError,
    routes::stream::{etag, not_modified, stream},
    state::AppState,
    storage,
};

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
    let etag = etag(&opened.fingerprint);
    if let Some(response) = not_modified(&headers, &etag) {
        return Ok(response);
    }

    let mime = opened.mime().await?;
    let body = Body::from_stream(ReaderStream::new(opened.file));
    Ok(stream(body, opened.size, mime, etag))
}
