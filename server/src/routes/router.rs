use axum::Router;

use super::{health, storage};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(storage::router())
        .with_state(state)
}
