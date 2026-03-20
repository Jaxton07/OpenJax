//! Protocol types and constants for the JSONL stdio daemon.

use serde::{Deserialize, Serialize};
use tracing::info;
use serde_json::Value;

pub const PROTOCOL_VERSION: &str = "v1";
pub const KIND_REQUEST: &str = "request";
pub const KIND_RESPONSE: &str = "response";
pub const KIND_EVENT: &str = "event";

#[derive(Debug, Deserialize)]
pub struct RequestEnvelope {
    pub protocol_version: String,
    pub kind: String,
    pub request_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct ResponseEnvelope {
    pub protocol_version: &'static str,
    pub kind: &'static str,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct EventEnvelope {
    pub protocol_version: &'static str,
    pub kind: &'static str,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub retriable: bool,
    pub details: Value,
}

pub struct ApprovalLogEvent<'a> {
    pub action: &'a str,
    pub request_id: Option<&'a str>,
    pub turn_id: Option<&'a str>,
    pub target: Option<&'a str>,
    pub approved: Option<bool>,
    pub resolved: Option<bool>,
    pub session_id: Option<&'a str>,
    pub detail: Option<&'a str>,
}

pub fn log_approval_event(event: ApprovalLogEvent<'_>) {
    info!(
        "approval_event action={} request_id={} turn_id={} target={} approved={} resolved={} session_id={} detail={}",
        approval_text_field(Some(event.action)),
        approval_text_field(event.request_id),
        approval_text_field(event.turn_id),
        approval_text_field(event.target),
        approval_bool_field(event.approved),
        approval_bool_field(event.resolved),
        approval_text_field(event.session_id),
        approval_text_field(event.detail),
    );
}

pub fn approval_bool_field(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "-",
    }
}

pub fn approval_text_field(value: Option<&str>) -> String {
    let raw = value.unwrap_or("-").trim();
    if raw.is_empty() {
        return "-".to_string();
    }
    raw.split_whitespace().collect::<Vec<_>>().join("_")
}
