use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};

use super::cursor::{decode_cursor, encode_cursor};
use crate::{
    error::AppError,
    state::AppState,
    storage::{self, Entry, EntryKey},
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
}

/// `GET /storage/ls?path=/a/b/c&limit=30&cursor=…`: a page of the directory's
/// entries, directories first, then by name ignoring case.
pub async fn ls(
    State(state): State<AppState>,
    Query(query): Query<LsQuery>,
) -> Result<Json<LsResponse>, AppError> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;

    let dir = storage::resolve(&state.config.storage, &query.path).await?;
    let previews = state.previews.clone();
    let page = storage::list_dir(&dir, after, limit, move |path, modified| {
        previews.is_fresh(path, modified)
    })
    .await?;
    let next_cursor = match page.entries.last() {
        Some(last) if page.has_more => Some(encode_cursor(&EntryKey::of(last))),
        _ => None,
    };

    Ok(Json(LsResponse {
        items: page.entries,
        next_cursor,
        total: page.total,
    }))
}
