pub mod provider;
pub mod session;
pub mod slash_commands;
pub mod stream;

pub use provider::*;
pub use session::*;
pub use slash_commands::{exec_slash_command, list_slash_commands};
pub use stream::*;

use axum::Json;
use axum::response::IntoResponse;
use serde_json::json;

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}
