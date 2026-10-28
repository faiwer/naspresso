use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{
    error::AppError,
    state::AppState,
    storage::{self, Entry},
};

#[derive(Deserialize)]
pub struct LsQuery {
    /// Directory inside `STORAGE`, e.g. `/a/b/c`. Defaults to the root.
    #[serde(default)]
    path: String,
}

/// `GET /storage/ls?path=/a/b/c`: the directory's entries, directories first, then by name.
pub async fn ls(
    State(state): State<AppState>,
    Query(query): Query<LsQuery>,
) -> Result<Json<Vec<Entry>>, AppError> {
    let dir = storage::resolve(&state.config.storage, &query.path).await?;
    Ok(Json(storage::list_dir(&dir).await?))
}
