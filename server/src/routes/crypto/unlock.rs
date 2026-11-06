use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    crypto::{self, Probe},
    error::{AppError, ClientError, ClientErrorCode},
    routes::cookie::set_cookie,
    state::AppState,
    storage,
};

/// How long an unlock cookie and a decrypted file response may live.
pub(crate) const COOKIE_MAX_AGE: i64 = 15 * 60;

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
    error: Option<ClientError>,
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
        return Ok(rejected(
            ClientErrorCode::NoEncryptedFile,
            "This folder has no encrypted file to open.",
        ));
    };

    let password = body.password;
    // Argon2id takes hundreds of milliseconds. Keep it off the runtime thread.
    let key = tokio::task::spawn_blocking(move || ::crypto::prepare(&password))
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))??;

    if crypto::probe(&file, &key).await? == Probe::Rejected {
        return Ok(rejected(ClientErrorCode::WrongPassword, "Wrong password."));
    }

    let expired_at = Utc::now() + Duration::seconds(COOKIE_MAX_AGE);
    let value = crypto::seal(state.config.npe_seal_key.as_bytes(), &key, expired_at)?;
    let mut response = Json(UnlockResponse {
        expired_at: Some(expired_at),
        unlocked: true,
        error: None,
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        set_cookie(
            &crypto::encode_cookie_name(&body.path),
            &value,
            &state.config.api_prefix,
            COOKIE_MAX_AGE,
        )?,
    );
    Ok(response)
}

fn rejected(code: ClientErrorCode, message: &str) -> Response {
    Json(UnlockResponse {
        expired_at: None,
        unlocked: false,
        error: Some(ClientError::new(code, message)),
    })
    .into_response()
}
