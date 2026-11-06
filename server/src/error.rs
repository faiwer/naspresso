use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use crypto::{ContentError, PasswordError};
use media::preview::PreviewError;
use serde::Serialize;
use thiserror::Error;

use crate::{
    cancel::Cancelled, crypto::CryptoError, preview::PreviewCacheError, storage::StorageError,
};

/// Code sent to the client. The variant name is the wire value (`"WrongPassword"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ClientErrorCode {
    NoEncryptedFile,
    WrongPassword,
    InvalidPath,
    NotADirectory,
    NotAFile,
    NotEncryptedDirectory,
    InvalidPassword,
    InvalidContent,
    InvalidSize,
    CannotDecrypt,
    BadRequest,
    NotFound,
    RangeNotSupported,
    UnsupportedMediaType,
    NotImplemented,
    Cancelled,
    Internal,
}

/// `{ code, message }` body. `message` is the sentence a client shows when it
/// has no copy for `code`.
#[derive(Debug, Serialize)]
pub struct ClientError {
    pub code: ClientErrorCode,
    pub message: String,
}

impl ClientError {
    pub(crate) fn new(code: ClientErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Top-level error type for handlers. Responds with a [`ClientError`].
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest {
        code: ClientErrorCode,
        message: String,
    },

    #[error("not found")]
    NotFound,

    #[error("range requests are not supported")]
    RangeNotSupported,

    #[error("{0}")]
    UnsupportedMediaType(String),

    #[error("{0}")]
    NotImplemented(String),

    /// The client went away; nobody reads the response.
    #[error("cancelled")]
    Cancelled,

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    /// A 400 whose `code` is finer than [`ClientErrorCode::BadRequest`] when
    /// the cause has its own variant.
    pub(crate) fn bad_request(code: ClientErrorCode, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            message: message.into(),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::RangeNotSupported => StatusCode::RANGE_NOT_SATISFIABLE,
            AppError::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            AppError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            // nginx's "client closed request".
            AppError::Cancelled => StatusCode::from_u16(499).expect("499 is a valid status"),
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> ClientErrorCode {
        match self {
            AppError::BadRequest { code, .. } => *code,
            AppError::NotFound => ClientErrorCode::NotFound,
            AppError::RangeNotSupported => ClientErrorCode::RangeNotSupported,
            AppError::UnsupportedMediaType(_) => ClientErrorCode::UnsupportedMediaType,
            AppError::NotImplemented(_) => ClientErrorCode::NotImplemented,
            AppError::Cancelled => ClientErrorCode::Cancelled,
            AppError::Internal(_) => ClientErrorCode::Internal,
        }
    }

    fn into_client_error(self) -> ClientError {
        let code = self.code();
        let message = match self {
            AppError::Internal(_) => "internal server error".to_owned(),
            AppError::BadRequest { message, .. } => message,
            other => other.to_string(),
        };
        ClientError::new(code, message)
    }
}

impl From<PasswordError> for AppError {
    fn from(err: PasswordError) -> Self {
        match err {
            PasswordError::Length | PasswordError::Symbol => {
                AppError::bad_request(ClientErrorCode::InvalidPassword, err.to_string())
            }
            PasswordError::Derive => AppError::Internal(err.into()),
        }
    }
}

impl From<CryptoError> for AppError {
    fn from(err: CryptoError) -> Self {
        match err {
            CryptoError::NotEncryptedDirectory => {
                AppError::bad_request(ClientErrorCode::NotEncryptedDirectory, err.to_string())
            }
            CryptoError::Storage(err) => err.into(),
            CryptoError::Seal => AppError::Internal(err.into()),
            CryptoError::Content(error) => match error {
                ContentError::Io(err) => AppError::Internal(err.into()),
                ContentError::Authentication => {
                    AppError::Internal(anyhow::anyhow!("encrypted content failed authentication"))
                }
                other => AppError::bad_request(ClientErrorCode::InvalidContent, other.to_string()),
            },
        }
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::InvalidPath(_) => {
                AppError::bad_request(ClientErrorCode::InvalidPath, err.to_string())
            }
            StorageError::NotADirectory => {
                AppError::bad_request(ClientErrorCode::NotADirectory, err.to_string())
            }
            StorageError::NotAFile => {
                AppError::bad_request(ClientErrorCode::NotAFile, err.to_string())
            }
            StorageError::NotFound => AppError::NotFound,
            StorageError::Io(e) => AppError::Internal(e.into()),
        }
    }
}

impl From<PreviewError> for AppError {
    fn from(err: PreviewError) -> Self {
        match err {
            PreviewError::InvalidSize(_) => {
                AppError::bad_request(ClientErrorCode::InvalidSize, err.to_string())
            }
            PreviewError::UnsupportedFormat(_) | PreviewError::Decode(_) => {
                AppError::UnsupportedMediaType(err.to_string())
            }
            PreviewError::HdrUnsupported => AppError::NotImplemented(err.to_string()),
            PreviewError::Encode(_) => AppError::Internal(anyhow::anyhow!(err)),
            PreviewError::Io(e) => AppError::Internal(e.into()),
        }
    }
}

impl From<PreviewCacheError> for AppError {
    fn from(err: PreviewCacheError) -> Self {
        match err {
            PreviewCacheError::Preview(e) => e.into(),
            PreviewCacheError::Cancelled(Cancelled) => AppError::Cancelled,
            PreviewCacheError::Decrypt => {
                AppError::bad_request(ClientErrorCode::CannotDecrypt, err.to_string())
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // The body stays generic for internal errors so implementation details don't leak.
        if let AppError::Internal(ref err) = self {
            tracing::error!(error = %err, "internal server error");
        }

        let status = self.status();
        (status, Json(self.into_client_error())).into_response()
    }
}
