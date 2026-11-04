use axum::{
    Json,
    extract::State,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    crypto::{self, Probe},
    error::AppError,
    state::AppState,
    storage,
};

// 15 minutes. Cookie expiration time.
const MAX_AGE: i64 = 15 * 60;

#[derive(Deserialize)]
pub(super) struct UnlockRequest {
    password: String,
    path: String,
}

#[derive(Serialize)]
struct UnlockResponse {
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    expired_at: Option<DateTime<Utc>>,
    unlocked: bool,
    error: Option<String>,
}

/// `POST /crypto/unlock`. A directory that is missing or not `.nped` is an HTTP
/// error. A folder with no `.npe` file, or a password that does not open one,
/// is `200` with `unlocked: false`.
pub(super) async fn unlock(
    State(state): State<AppState>,
    Json(body): Json<UnlockRequest>,
) -> Result<Response, AppError> {
    let dir = storage::resolve(&state.config.storage, &body.path).await?;
    crypto::ensure_encrypted_dir(&dir).await?;
    ::crypto::validate_password(&body.password)?;

    let Some(file) = crypto::find_encrypted_file(&dir).await? else {
        return Ok(rejected("no encrypted file"));
    };

    let password = body.password;
    // Argon2id takes hundreds of milliseconds. Keep it off the runtime thread.
    let key = tokio::task::spawn_blocking(move || ::crypto::prepare(&password))
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))??;

    if crypto::probe(&file, &key).await? == Probe::Rejected {
        return Ok(rejected("wrong password"));
    }

    let value = crypto::seal(state.config.npe_seal_key.as_bytes(), &key)?;
    let mut response = Json(UnlockResponse {
        expired_at: Some(Utc::now() + Duration::seconds(MAX_AGE)),
        unlocked: true,
        error: None,
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie_header(
            &crypto::encode_cookie_name(&body.path),
            &value,
            &state.config.api_prefix,
        )?,
    );
    Ok(response)
}

fn rejected(error: &str) -> Response {
    Json(UnlockResponse {
        expired_at: None,
        unlocked: false,
        error: Some(error.to_owned()),
    })
    .into_response()
}

fn cookie_header(name: &str, value: &str, path: &str) -> Result<HeaderValue, AppError> {
    let header = format!("{name}={value}; HttpOnly; SameSite=Lax; Path={path}; Max-Age={MAX_AGE}");
    HeaderValue::from_str(&header).map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
