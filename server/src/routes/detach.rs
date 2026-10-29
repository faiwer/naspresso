use axum::{
    extract::{FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{cancel::Cancel, error::AppError};

/// Runs every handler to the end in its own task, so a client disconnect only
/// discards the response. Handlers that want to give up take a [`Cancel`].
pub async fn detach(mut req: Request, next: Next) -> Response {
    let cancel = Cancel::default();
    req.extensions_mut().insert(cancel.clone());
    // hyper drops this future, not the task, when the client disconnects.
    let _cancel_on_drop = cancel.cancel_on_drop();
    match tokio::spawn(next.run(req)).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!(error = %e, "handler panicked");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

impl<S: Sync> FromRequestParts<S> for Cancel {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Cancel>()
            .cloned()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("the detach layer is missing")))
    }
}
