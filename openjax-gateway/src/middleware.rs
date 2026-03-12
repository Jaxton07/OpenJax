use axum::extract::{MatchedPath, Request, State};
use axum::http::HeaderValue;
use axum::http::header::{AUTHORIZATION, HeaderName};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{ApiError, ResultCode};
use crate::state::AppState;

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

tokio::task_local! {
    static REQUEST_CONTEXT: RequestContext;
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub actor: String,
}

impl RequestContext {
    pub fn current_request_id() -> Option<String> {
        REQUEST_CONTEXT.try_with(|ctx| ctx.request_id.clone()).ok()
    }
}

pub async fn request_context_middleware(
    State(_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let request_id = req
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("req_{}", Uuid::new_v4().simple()));

    let ctx = RequestContext {
        request_id: request_id.clone(),
        actor: "anonymous".to_string(),
    };
    req.extensions_mut().insert(ctx.clone());

    REQUEST_CONTEXT
        .scope(ctx, async move {
            let mut response = next.run(req).await;
            if let Ok(header_value) = HeaderValue::from_str(&request_id) {
                response.headers_mut().insert(X_REQUEST_ID, header_value);
            }
            response
        })
        .await
}

pub async fn owner_key_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let header_value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthenticated("missing Authorization header"))?;

    let token = crate::auth::parse_bearer_token(header_value)
        .ok_or_else(|| ApiError::unauthenticated("invalid bearer token"))?;
    if !state.is_api_key_allowed(token) {
        return Err(ApiError::unauthenticated("api key is not allowed"));
    }

    let actor = format!("api_key:{}", &token.chars().take(6).collect::<String>());
    if let Some(ctx) = req.extensions_mut().get_mut::<RequestContext>() {
        ctx.actor = actor;
    }

    Ok(next.run(req).await)
}

pub async fn access_token_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let header_value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthenticated("missing Authorization header"))?;

    let token = crate::auth::parse_bearer_token(header_value)
        .ok_or_else(|| ApiError::unauthenticated("invalid bearer token"))?;

    let session = state
        .auth_service()
        .validate_access_token(token)
        .ok_or_else(|| ApiError::unauthenticated("access token is invalid or expired"))?;

    let actor = format!("auth_session:{}", session.session_id);
    if let Some(ctx) = req.extensions_mut().get_mut::<RequestContext>() {
        ctx.actor = actor;
    }

    Ok(next.run(req).await)
}

pub async fn access_log_middleware(
    State(_state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unknown")
        .to_string();
    let method = req.method().to_string();
    let request_id = req
        .extensions()
        .get::<RequestContext>()
        .map(|ctx| ctx.request_id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let session_id = req
        .uri()
        .path()
        .split('/')
        .find(|segment| segment.starts_with("sess_"))
        .unwrap_or("-")
        .to_string();
    let started = Instant::now();
    let response = next.run(req).await;
    let latency_ms = started.elapsed().as_millis();
    let result_code = response
        .extensions()
        .get::<ResultCode>()
        .map(|rc| rc.0.clone())
        .unwrap_or_else(|| {
            if response.status().is_success() {
                "OK".to_string()
            } else {
                format!("HTTP_{}", response.status().as_u16())
            }
        });

    if response.status().is_server_error() {
        warn!(
            request_id = %request_id,
            session_id = %session_id,
            route = %route,
            method = %method,
            latency_ms = latency_ms,
            result_code = %result_code,
            "gateway request completed with server error"
        );
    } else {
        info!(
            request_id = %request_id,
            session_id = %session_id,
            route = %route,
            method = %method,
            latency_ms = latency_ms,
            result_code = %result_code,
            "gateway request completed"
        );
    }

    response
}
