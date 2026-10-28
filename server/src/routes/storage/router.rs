use axum::{Router, routing::get};

use super::ls;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/storage/ls", get(ls::ls))
}
