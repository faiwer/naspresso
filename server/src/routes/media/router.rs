use axum::{Router, routing::get};

use super::preview;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/media/preview/{size}/{*file}", get(preview::preview))
}
