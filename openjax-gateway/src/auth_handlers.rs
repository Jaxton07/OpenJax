use axum::Json;
use axum::extract::{Extension, State};
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::cookie::{
    build_refresh_clear_cookie, build_refresh_set_cookie, parse_cookie_value, refresh_cookie_name,
};
use crate::auth::{NewSessionInput, RefreshError, parse_bearer_token};
use crate::error::{ApiError, now_rfc3339};
use crate::middleware::RequestContext;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    device_name: Option<String>,
    platform: Option<String>,
    user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    session_id: Option<String>,
    device_id: Option<String>,
    revoke_all: Option<bool>,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    request_id: String,
    access_token: String,
    access_expires_in: u64,
    session_id: String,
    scope: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct LogoutResponse {
    request_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct RevokeResponse {
    request_id: String,
    revoked: usize,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct SessionsResponse {
    request_id: String,
    sessions: Vec<crate::auth::SessionView>,
    timestamp: String,
}

pub async fn login(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let owner_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthenticated("missing owner Authorization header"))?;
    let owner_key = parse_bearer_token(owner_header)
        .ok_or_else(|| ApiError::unauthenticated("invalid owner bearer token"))?;

    if !state.auth_service().allow_login(owner_key) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "RATE_LIMITED",
            message: "login rate limited".to_string(),
            retryable: true,
            details: json!({ "action": "login" }),
        });
    }

    let result = state
        .auth_service()
        .login(NewSessionInput {
            device_name: payload.device_name,
            platform: payload.platform,
            user_agent: payload.user_agent,
        })
        .map_err(|_| ApiError::internal("failed to create auth session"))?;

    let cookie = build_refresh_set_cookie(
        &result.refresh_token,
        state.auth_service().config().cookie_secure,
        state.auth_service().config().refresh_ttl_seconds(),
    );

    let mut response = Json(LoginResponse {
        request_id: ctx.request_id,
        access_token: result.access_token,
        access_expires_in: result.access_expires_in,
        session_id: result.session.session_id,
        scope: result.session.scope,
        timestamp: now_rfc3339(),
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    Ok(response)
}

pub async fn refresh(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let refresh_token = extract_refresh_token(&headers, payload.refresh_token)
        .ok_or_else(|| ApiError::unauthenticated("missing refresh token"))?;

    if !state
        .auth_service()
        .allow_refresh(&refresh_token.chars().take(12).collect::<String>())
    {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "RATE_LIMITED",
            message: "refresh rate limited".to_string(),
            retryable: true,
            details: json!({ "action": "refresh" }),
        });
    }

    let refreshed = match state.auth_service().refresh(&refresh_token) {
        Ok(value) => value,
        Err(RefreshError::Missing) => {
            return Err(ApiError::unauthenticated(
                "refresh token is invalid or expired",
            ));
        }
        Err(RefreshError::ReuseDetected) => {
            return Err(ApiError::conflict(
                "refresh token reuse detected",
                json!({ "action": "refresh" }),
            ));
        }
    };

    let cookie = build_refresh_set_cookie(
        &refreshed.refresh_token,
        state.auth_service().config().cookie_secure,
        state.auth_service().config().refresh_ttl_seconds(),
    );

    let mut response = Json(LoginResponse {
        request_id: ctx.request_id,
        access_token: refreshed.access_token,
        access_expires_in: refreshed.access_expires_in,
        session_id: refreshed.session.session_id,
        scope: refreshed.session.scope,
        timestamp: now_rfc3339(),
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    Ok(response)
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<LogoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session_id = payload.session_id.ok_or_else(|| {
        ApiError::invalid_argument("session_id is required", json!({ "field": "session_id" }))
    })?;

    let _ = state.auth_service().logout_by_session(&session_id);

    let clear_cookie = build_refresh_clear_cookie(state.auth_service().config().cookie_secure);
    let mut response = Json(LogoutResponse {
        request_id: ctx.request_id,
        status: "logged_out",
        timestamp: now_rfc3339(),
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    Ok(response)
}

pub async fn revoke(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<RevokeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let revoked = state
        .auth_service()
        .revoke(
            payload.session_id.as_deref(),
            payload.device_id.as_deref(),
            payload.revoke_all.unwrap_or(false),
        )
        .map_err(|_| ApiError::internal("failed to revoke sessions"))?;

    Ok(Json(RevokeResponse {
        request_id: ctx.request_id,
        revoked,
        timestamp: now_rfc3339(),
    }))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let sessions = state
        .auth_service()
        .list_sessions()
        .map_err(|_| ApiError::internal("failed to list auth sessions"))?;
    Ok(Json(SessionsResponse {
        request_id: ctx.request_id,
        sessions,
        timestamp: now_rfc3339(),
    }))
}

fn extract_refresh_token(headers: &HeaderMap, body_token: Option<String>) -> Option<String> {
    if let Some(value) = headers.get(COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(token) = parse_cookie_value(value, refresh_cookie_name()) {
            return Some(token);
        }
    }
    body_token
        .map(|value| value.trim().to_string())
        .filter(|v| !v.is_empty())
}
