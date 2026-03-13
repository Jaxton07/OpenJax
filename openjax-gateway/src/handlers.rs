use std::convert::Infallible;
use std::time::Duration;

use async_stream::stream;
use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{ApiError, now_rfc3339};
use crate::middleware::RequestContext;
use crate::state::{
    ApiTurnError, AppState, SessionStatus, StreamEventEnvelope, TurnRuntime, TurnStatus,
    run_turn_task,
};

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    request_id: String,
    session_id: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitTurnResponse {
    request_id: String,
    session_id: String,
    turn_id: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct TurnResponse {
    request_id: String,
    session_id: String,
    turn_id: String,
    status: TurnStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ApiTurnError>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ApprovalResolveResponse {
    request_id: String,
    session_id: String,
    approval_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SessionActionResponse {
    request_id: String,
    session_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTurnRequest {
    input: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveApprovalRequest {
    approved: bool,
    #[allow(dead_code)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    after_event_seq: Option<u64>,
    protocol: Option<String>,
}

fn resolve_resume_seq(after_event_seq: Option<u64>, last_event_id: Option<&str>) -> Option<u64> {
    after_event_seq.or_else(|| last_event_id.and_then(|value| value.parse::<u64>().ok()))
}

fn sse_protocol_v2_enabled() -> bool {
    let raw = std::env::var("OPENJAX_SSE_PROTOCOL_V2")
        .ok()
        .unwrap_or_else(|| "1".to_string());
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "0" | "off" | "false" | "disabled"
    )
}

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> impl IntoResponse {
    Json(json!({ "status": "ready" }))
}

pub async fn create_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    let session_id = state.create_session().await;
    Ok(Json(CreateSessionResponse {
        request_id: ctx.request_id,
        session_id,
        timestamp: now_rfc3339(),
    }))
}

pub async fn submit_turn(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<SubmitTurnRequest>,
) -> Result<Json<SubmitTurnResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let input = payload.input.trim().to_string();

    {
        let session = session_runtime.lock().await;
        if matches!(
            session.status,
            SessionStatus::Closed | SessionStatus::Closing
        ) {
            return Err(ApiError::conflict(
                "session is not active",
                json!({ "session_id": session_id }),
            ));
        }
    }

    if input == "/compact" {
        return Err(ApiError::not_implemented(
            "compact is not implemented yet",
            json!({ "session_id": session_id }),
        ));
    }
    if input == "/clear" {
        clear_runtime(&session_runtime).await;
        let turn_id = format!("turn_cmd_{}", Uuid::new_v4().simple());
        let mut session = session_runtime.lock().await;
        session.turns.insert(
            turn_id.clone(),
            TurnRuntime {
                status: TurnStatus::Completed,
                assistant_message: Some("session cleared".to_string()),
                error: None,
            },
        );
        let started = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "turn_started",
            json!({}),
            None,
        );
        let message = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "assistant_message",
            json!({ "content": "session cleared" }),
            None,
        );
        let completed = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            Some(turn_id.clone()),
            "turn_completed",
            json!({}),
            None,
        );
        session.publish_event(started);
        session.publish_event(message);
        session.publish_event(completed);
        return Ok(Json(SubmitTurnResponse {
            request_id: ctx.request_id,
            session_id,
            turn_id,
            timestamp: now_rfc3339(),
        }));
    }

    let (turn_id_tx, turn_id_rx) = oneshot::channel();
    tokio::spawn(run_turn_task(
        session_runtime,
        session_id.clone(),
        ctx.request_id.clone(),
        input,
        turn_id_tx,
    ));

    let turn_id = match timeout(Duration::from_secs(20), turn_id_rx).await {
        Ok(Ok(Ok(turn_id))) => turn_id,
        Ok(Ok(Err(err))) => return Err(err),
        Ok(Err(_)) => {
            return Err(ApiError::upstream_unavailable(
                "turn worker stopped unexpectedly",
                json!({ "session_id": session_id }),
            ));
        }
        Err(_) => {
            return Err(ApiError::timeout(
                "timed out waiting for turn id",
                json!({ "session_id": session_id }),
            ));
        }
    };

    Ok(Json(SubmitTurnResponse {
        request_id: ctx.request_id,
        session_id,
        turn_id,
        timestamp: now_rfc3339(),
    }))
}

pub async fn get_turn(
    State(state): State<AppState>,
    Path((session_id, turn_id)): Path<(String, String)>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<TurnResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let session = session_runtime.lock().await;
    let turn = session.turns.get(&turn_id).ok_or_else(|| {
        ApiError::not_found(
            "turn not found",
            json!({ "session_id": session_id, "turn_id": turn_id }),
        )
    })?;
    Ok(Json(TurnResponse {
        request_id: ctx.request_id,
        session_id,
        turn_id,
        status: turn.status,
        assistant_message: turn.assistant_message.clone(),
        error: turn.error.clone(),
        timestamp: now_rfc3339(),
    }))
}

pub async fn resolve_approval(
    State(state): State<AppState>,
    Path((session_id, approval_action)): Path<(String, String)>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<ResolveApprovalRequest>,
) -> Result<Json<ApprovalResolveResponse>, ApiError> {
    let approval_action = approval_action.trim_start_matches('/');
    let approval_id = approval_action
        .strip_suffix(":resolve")
        .ok_or_else(|| {
            ApiError::not_found("invalid approval route", json!({ "path": approval_action }))
        })?
        .to_string();

    let session_runtime = state.get_session(&session_id).await?;
    let approval_handler = {
        let session = session_runtime.lock().await;
        if session.resolved_approvals.contains(&approval_id) {
            return Err(ApiError::conflict(
                "approval already resolved",
                json!({ "approval_id": approval_id }),
            ));
        }
        session.approval_handler.clone()
    };

    if !approval_handler
        .resolve(&approval_id, payload.approved)
        .await
    {
        return Err(ApiError::not_found(
            "approval not found",
            json!({ "approval_id": approval_id }),
        ));
    }

    let mut session = session_runtime.lock().await;
    session.resolved_approvals.insert(approval_id.clone());
    info!(
        request_id = %ctx.request_id,
        session_id = %session_id,
        approval_id = %approval_id,
        approved = payload.approved,
        actor = %ctx.actor,
        "approval resolved"
    );

    Ok(Json(ApprovalResolveResponse {
        request_id: ctx.request_id,
        session_id,
        approval_id,
        status: "resolved",
        timestamp: now_rfc3339(),
    }))
}

pub async fn session_action(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionActionResponse>, ApiError> {
    let (session_id, action) = parse_session_action(&session_id)?;
    let normalized = normalize_session_action(action);
    if normalized == "compact" {
        return Err(ApiError::not_implemented(
            "compact is not implemented yet",
            json!({ "session_id": session_id }),
        ));
    }
    if normalized != "clear" {
        return Err(ApiError::not_found(
            "invalid session action route",
            json!({ "action": action }),
        ));
    }
    let session_runtime = state.get_session(session_id).await?;
    clear_runtime(&session_runtime).await;
    Ok(Json(SessionActionResponse {
        request_id: ctx.request_id,
        session_id: session_id.to_string(),
        status: "cleared",
        timestamp: now_rfc3339(),
    }))
}

pub async fn shutdown_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<SessionActionResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;

    {
        let mut session = session_runtime.lock().await;
        let event = session.create_gateway_event(
            &ctx.request_id,
            &session_id,
            None,
            "session_shutdown",
            json!({ "reason": "shutdown_requested" }),
            None,
        );
        session.publish_event(event);
        session.status = SessionStatus::Closing;
        let agent = session.agent.clone();
        drop(session);
        let mut agent = agent.lock().await;
        let _ = agent.submit(openjax_protocol::Op::Shutdown).await;
        let mut session = session_runtime.lock().await;
        session.status = SessionStatus::Closed;
    }
    state.remove_session(&session_id).await;

    Ok(Json(SessionActionResponse {
        request_id: ctx.request_id,
        session_id,
        status: "shutdown",
        timestamp: now_rfc3339(),
    }))
}

pub async fn stream_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let (replay, mut rx) = {
        let session = session_runtime.lock().await;
        let last_event_id = headers
            .get("Last-Event-ID")
            .and_then(|value| value.to_str().ok());
        let after_event_seq = resolve_resume_seq(query.after_event_seq, last_event_id);
        (
            session.replay_from(after_event_seq)?,
            session.event_tx.subscribe(),
        )
    };

    let requested_protocol = query.protocol.clone().unwrap_or_else(|| "v1".to_string());
    let protocol = if requested_protocol.eq_ignore_ascii_case("v2") && sse_protocol_v2_enabled() {
        "v2".to_string()
    } else {
        "v1".to_string()
    };
    let session_runtime_for_recovery = session_runtime.clone();
    let request_id = headers
        .get("X-Request-Id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("req_stream")
        .to_string();
    let event_stream = stream! {
        let mut last_sent_event_seq = replay.last().map(|event| event.event_seq).unwrap_or(0);
        for event in replay {
            last_sent_event_seq = event.event_seq;
            yield Ok(to_sse_event(event, &protocol));
        }
        loop {
            match rx.recv().await {
                Ok(event) => {
                    last_sent_event_seq = event.event_seq;
                    yield Ok(to_sse_event(event, &protocol));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let recovered = {
                        let session = session_runtime_for_recovery.lock().await;
                        session.replay_from(Some(last_sent_event_seq))
                    };
                    match recovered {
                        Ok(missed) if !missed.is_empty() => {
                            info!(
                                session_id = %session_id,
                                recovered_count = missed.len(),
                                last_sent_event_seq = last_sent_event_seq,
                                "sse_lagged_recovered"
                            );
                            for event in missed {
                                if event.event_seq <= last_sent_event_seq {
                                    continue;
                                }
                                last_sent_event_seq = event.event_seq;
                                yield Ok(to_sse_event(event, &protocol));
                            }
                        }
                        _ => {
                            warn!(
                                session_id = %session_id,
                                last_sent_event_seq = last_sent_event_seq,
                                "sse_lagged_recovery_failed"
                            );
                            let recovery_error = StreamEventEnvelope {
                                request_id: request_id.clone(),
                                session_id: session_id.clone(),
                                turn_id: None,
                                event_seq: last_sent_event_seq.saturating_add(1),
                                turn_seq: 0,
                                timestamp: now_rfc3339(),
                                event_type: "response_error".to_string(),
                                stream_source: "replay".to_string(),
                                payload: json!({
                                    "code": "REPLAY_WINDOW_EXCEEDED",
                                    "message": "event replay window exceeded; reconnect required",
                                    "retryable": true
                                }),
                            };
                            yield Ok(to_sse_event(recovery_error, &protocol));
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

fn to_sse_event(event: StreamEventEnvelope, protocol: &str) -> SseEvent {
    let adapted = if protocol.eq_ignore_ascii_case("v2") {
        adapt_event_v2(event)
    } else {
        event
    };
    SseEvent::default()
        .event(adapted.event_type.clone())
        .id(adapted.event_seq.to_string())
        .data(serde_json::to_string(&adapted).unwrap_or_else(|_| "{}".to_string()))
}

fn adapt_event_v2(mut event: StreamEventEnvelope) -> StreamEventEnvelope {
    let mapped = match event.event_type.as_str() {
        "turn_started" => "response_started",
        "assistant_delta" => "response_text_delta",
        "assistant_message" | "turn_completed" => "response_completed",
        other => other,
    };
    event.event_type = mapped.to_string();
    event
}

#[cfg(test)]
mod tests {
    use super::{adapt_event_v2, resolve_resume_seq};
    use crate::state::StreamEventEnvelope;
    use serde_json::json;

    fn base_event(event_type: &str) -> StreamEventEnvelope {
        StreamEventEnvelope {
            request_id: "req_1".to_string(),
            session_id: "sess_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            event_seq: 1,
            turn_seq: 1,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            event_type: event_type.to_string(),
            stream_source: "synthetic".to_string(),
            payload: json!({}),
        }
    }

    #[test]
    fn adapt_event_v2_maps_assistant_delta_to_response_text_delta() {
        let event = adapt_event_v2(base_event("assistant_delta"));
        assert_eq!(event.event_type, "response_text_delta");
    }

    #[test]
    fn adapt_event_v2_maps_turn_completed_to_response_completed() {
        let event = adapt_event_v2(base_event("turn_completed"));
        assert_eq!(event.event_type, "response_completed");
    }

    #[test]
    fn resolve_resume_seq_prefers_after_event_seq() {
        assert_eq!(resolve_resume_seq(Some(9), Some("3")), Some(9));
    }

    #[test]
    fn resolve_resume_seq_uses_last_event_id_when_query_absent() {
        assert_eq!(resolve_resume_seq(None, Some("7")), Some(7));
    }
}

fn normalize_session_action(action: &str) -> String {
    action
        .trim_start_matches('/')
        .trim_start_matches(':')
        .to_string()
}

fn parse_session_action(session_action: &str) -> Result<(&str, &str), ApiError> {
    session_action.split_once(':').ok_or_else(|| {
        ApiError::not_found(
            "invalid session action route",
            json!({ "session_action": session_action }),
        )
    })
}

async fn clear_runtime(session_runtime: &tokio::sync::Mutex<crate::state::SessionRuntime>) {
    let mut session = session_runtime.lock().await;
    session.clear_context();
}
