mod auth;
mod error;
mod handlers;
mod middleware;
pub mod state;

pub use state::AppState;

use std::path::PathBuf;

use axum::Router;
use axum::http::Method;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

pub fn build_app(state: AppState, static_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse().expect("valid origin"),
            "http://127.0.0.1:5173".parse().expect("valid origin"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    let protected = Router::new()
        .route("/api/v1/sessions", post(handlers::create_session))
        .route(
            "/api/v1/sessions/:session_id",
            post(handlers::session_action).delete(handlers::shutdown_session),
        )
        .route(
            "/api/v1/sessions/:session_id/turns",
            post(handlers::submit_turn),
        )
        .route(
            "/api/v1/sessions/:session_id/turns/:turn_id",
            get(handlers::get_turn),
        )
        .route(
            "/api/v1/sessions/:session_id/approvals/*approval_action",
            post(handlers::resolve_approval),
        )
        .route(
            "/api/v1/sessions/:session_id/events",
            get(handlers::stream_events),
        )
        .layer(from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let mut app = Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .merge(protected)
        .layer(from_fn_with_state(
            state.clone(),
            middleware::request_context_middleware,
        ))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::access_log_middleware,
        ))
        .layer(from_fn(error::error_catch_middleware))
        .layer(cors)
        .with_state(state);

    if let Some(static_dir) = static_dir {
        let index = static_dir.join("index.html");
        if index.is_file() {
            app = app
                .route_service("/", ServeFile::new(index))
                .nest_service("/assets", ServeDir::new(static_dir.join("assets")));
        }
    }

    app
}
