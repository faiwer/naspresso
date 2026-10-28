use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

use crate::storage::StorageError;

/// Top-level error type for handlers. Responds with `{ error, message }`, where
/// `error` is a stable machine-readable code.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("not found")]
    NotFound,

    #[error("range requests are not supported")]
    RangeNotSupported,

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::RangeNotSupported => StatusCode::RANGE_NOT_SATISFIABLE,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "bad_request",
            AppError::NotFound => "not_found",
            AppError::RangeNotSupported => "range_not_supported",
            AppError::Internal(_) => "internal_error",
        }
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::InvalidPath(_) | StorageError::NotADirectory | StorageError::NotAFile => {
                AppError::BadRequest(err.to_string())
            }
            StorageError::NotFound => AppError::NotFound,
            StorageError::Io(e) => AppError::Internal(e.into()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // The body stays generic for internal errors so implementation details don't leak.
        if let AppError::Internal(ref err) = self {
            tracing::error!(error = %err, "internal server error");
        }

        let message = match &self {
            AppError::Internal(_) => "internal server error".to_owned(),
            other => other.to_string(),
        };
        let body = ErrorBody {
            error: self.code(),
            message,
        };

        (self.status(), Json(body)).into_response()
    }
}
