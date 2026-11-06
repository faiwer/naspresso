use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{crypto, error::AppError, routes::cookie::set_cookie, state::AppState};

#[derive(Deserialize)]
pub(super) struct LockRequest {
    path: String,
}

/// `POST /crypto/lock`. Tells the browser to drop the unlock cookie for `path`.
/// A cookie that was never set is still `204`.
pub(super) async fn lock(
    State(state): State<AppState>,
    Json(body): Json<LockRequest>,
) -> Result<Response, AppError> {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        set_cookie(
            &crypto::encode_cookie_name(&body.path),
            "",
            &state.config.api_prefix,
            0,
        )?,
    );
    Ok(response)
}
