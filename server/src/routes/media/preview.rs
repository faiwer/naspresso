use axum::{
    body::Body,
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
};
use media::preview::PreviewSize;
use tokio_util::io::ReaderStream;

use crate::{
    cancel::Cancel,
    error::AppError,
    preview::Preview,
    routes::stream::{etag, not_modified, stream},
    state::AppState,
    storage,
};

/// `GET /media/preview/256/a/b/c.jpg`: an AVIF preview of the image whose
/// longest side is at most 128, 256 or 512. Revalidates with `If-None-Match`.
pub async fn preview(
    cancel: Cancel,
    State(state): State<AppState>,
    Path((size, file)): Path<(u32, String)>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let size = PreviewSize::try_from(size)?;
    let path = storage::resolve(&state.config.storage, &file).await?;
    let fingerprint = storage::open_file(&path).await?.fingerprint;
    let etag = etag(&fingerprint);
    if let Some(response) = not_modified(&headers, &etag) {
        return Ok(response);
    }

    let preview = state
        .previews
        .get(&path, &fingerprint, size, &cancel)
        .await?;
    Ok(match preview {
        Preview::Cached(avif) => stream(
            Body::from_stream(ReaderStream::new(avif.reader)),
            avif.len,
            "image/avif",
            etag,
        ),
        Preview::Fresh(avif) => {
            let len = avif.len() as u64;
            stream(Body::from(avif), len, "image/avif", etag)
        }
    })
}
