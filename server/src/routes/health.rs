use axum::{Json, Router, routing::get};
use serde::Serialize;

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "okay" })
}
