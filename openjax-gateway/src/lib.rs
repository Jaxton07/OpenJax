mod auth;
mod auth_handlers;
mod error;
mod event_mapper;
mod handlers;
mod middleware;
pub mod state;
pub mod stdio;

pub use auth::{ApiKeyConfig, ApiKeySource, load_api_keys};
pub use state::AppState;
pub use stdio::run_stdio;

use std::path::PathBuf;

use axum::Router;
use axum::http::Method;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{get, patch, post};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

pub fn build_app(state: AppState, static_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse().expect("valid origin"),
            "http://127.0.0.1:5173".parse().expect("valid origin"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            COOKIE,
            "x-request-id".parse().expect("header"),
        ])
        .allow_credentials(true);

    let login_auth = Router::new()
        .route("/api/v1/auth/login", post(auth_handlers::login))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::owner_key_middleware,
        ));

    let auth_protected = Router::new()
        .route("/api/v1/auth/sessions", get(auth_handlers::list_sessions))
        .route("/api/v1/auth/revoke", post(auth_handlers::revoke))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::access_token_middleware,
        ));

    let auth_public = Router::new()
        .route("/api/v1/auth/refresh", post(auth_handlers::refresh))
        .route("/api/v1/auth/logout", post(auth_handlers::logout));

    let protected = Router::new()
        .route(
            "/api/v1/sessions",
            post(handlers::create_session).get(handlers::list_sessions),
        )
        .route(
            "/api/v1/sessions/:session_id",
            post(handlers::session_action).delete(handlers::shutdown_session),
        )
        .route(
            "/api/v1/sessions/:session_id/messages",
            get(handlers::list_session_messages),
        )
        .route(
            "/api/v1/sessions/:session_id/timeline",
            get(handlers::list_session_timeline),
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
        .route(
            "/api/v1/providers",
            get(handlers::list_providers).post(handlers::create_provider),
        )
        .route(
            "/api/v1/providers/active",
            get(handlers::get_active_provider).put(handlers::set_active_provider),
        )
        .route(
            "/api/v1/providers/:provider_id",
            patch(handlers::update_provider).delete(handlers::delete_provider),
        )
        .layer(from_fn_with_state(
            state.clone(),
            middleware::access_token_middleware,
        ));

    let mut app = Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .merge(login_auth)
        .merge(auth_protected)
        .merge(auth_public)
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
                .route_service("/login", ServeFile::new(static_dir.join("index.html")))
                .route_service("/chat", ServeFile::new(static_dir.join("index.html")))
                .nest_service("/assets", ServeDir::new(static_dir.join("assets")));
        }
    }

    app
}
