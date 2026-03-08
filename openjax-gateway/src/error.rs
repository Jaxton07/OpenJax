use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, middleware::Next};
use serde::Serialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::middleware::RequestContext;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

impl ApiError {
    pub fn unauthenticated(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHENTICATED",
            message: message.into(),
            retryable: false,
            details: json!({}),
        }
    }

    pub fn invalid_argument(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_ARGUMENT",
            message: message.into(),
            retryable: false,
            details,
        }
    }

    pub fn not_found(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: message.into(),
            retryable: false,
            details,
        }
    }

    pub fn conflict(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "CONFLICT",
            message: message.into(),
            retryable: false,
            details,
        }
    }

    pub fn upstream_unavailable(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "UPSTREAM_UNAVAILABLE",
            message: message.into(),
            retryable: true,
            details,
        }
    }

    pub fn timeout(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            code: "TIMEOUT",
            message: message.into(),
            retryable: true,
            details,
        }
    }

    pub fn not_implemented(message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            code: "NOT_IMPLEMENTED",
            message: message.into(),
            retryable: false,
            details,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL",
            message: message.into(),
            retryable: false,
            details: json!({}),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    retryable: bool,
    details: Value,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    request_id: String,
    timestamp: String,
    error: ErrorBody,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id =
            RequestContext::current_request_id().unwrap_or_else(|| "unknown".to_string());
        let body = ErrorEnvelope {
            request_id,
            timestamp: now_rfc3339(),
            error: ErrorBody {
                code: self.code,
                message: self.message,
                retryable: self.retryable,
                details: self.details,
            },
        };
        let mut response = (self.status, Json(body)).into_response();
        response
            .extensions_mut()
            .insert(ResultCode(self.code.to_string()));
        response
    }
}

#[derive(Debug, Clone)]
pub struct ResultCode(pub String);

pub async fn error_catch_middleware(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    if response.status().is_server_error() && response.extensions().get::<ResultCode>().is_none() {
        let mut response = response;
        response
            .extensions_mut()
            .insert(ResultCode("INTERNAL".to_string()));
        return response;
    }
    response
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
