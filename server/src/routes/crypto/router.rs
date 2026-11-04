use axum::{Router, routing::post};

use super::unlock;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/crypto/unlock", post(unlock::unlock))
}
