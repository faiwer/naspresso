use axum::{Router, routing::get};

use super::{file, ls};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/storage/ls", get(ls::ls))
        .route("/storage/file", get(file::file))
}
