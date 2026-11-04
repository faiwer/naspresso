use axum::{Router, middleware};

use super::{crypto, detach::detach, health, media, storage};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(media::router())
        .merge(storage::router())
        .merge(crypto::router())
        .layer(middleware::from_fn(detach))
        .with_state(state)
}
