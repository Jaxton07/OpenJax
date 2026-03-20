pub mod session;
pub mod stream;
pub mod provider;

pub use session::*;
pub use stream::*;
pub use provider::*;

use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}
