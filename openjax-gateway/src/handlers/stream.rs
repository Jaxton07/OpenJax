use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::Duration;

use async_stream::stream;
use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::error::{ApiError, now_rfc3339};
use crate::state::{AppState, StreamEventEnvelope, append_then_publish, event_replay_limit};

static STREAM_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

fn gateway_stream_debug_enabled() -> bool {
    *STREAM_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("OPENJAX_GATEWAY_STREAM_DEBUG")
            .ok()
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                !(normalized == "0"
                    || normalized == "off"
                    || normalized == "false"
                    || normalized == "disabled")
            })
            .unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub after_event_seq: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SessionTimelineResponse {
    request_id: String,
    session_id: String,
    events: Vec<StreamEventEnvelope>,
    timestamp: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn resolve_resume_seq(
    after_event_seq: Option<u64>,
    last_event_id: Option<&str>,
) -> Option<u64> {
    after_event_seq.or_else(|| last_event_id.and_then(|value| value.parse::<u64>().ok()))
}

pub fn to_sse_event(event: StreamEventEnvelope) -> SseEvent {
    SseEvent::default()
        .event(event.event_type.clone())
        .id(event.event_seq.to_string())
        .data(serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string()))
}

fn should_emit_broadcast_event(last_sent_event_seq: u64, event_seq: u64) -> bool {
    event_seq > last_sent_event_seq
}

pub fn publish_event_for_session(
    state: &AppState,
    session: &mut crate::state::SessionRuntime,
    event: StreamEventEnvelope,
) {
    if let Err(err) = append_then_publish(state, session, event.clone()) {
        warn!(
            session_id = %event.session_id,
            event_seq = event.event_seq,
            event_type = %event.event_type,
            error = %err.message,
            "failed to append event before publish; event not broadcast"
        );
    }
}

fn replay_events_from_transcript(
    state: &AppState,
    session_id: &str,
    after_event_seq: Option<u64>,
) -> Result<Vec<StreamEventEnvelope>, ApiError> {
    let persisted = state.list_session_events(session_id, None)?;
    if persisted.is_empty() {
        return Ok(Vec::new());
    }

    let replay_limit = event_replay_limit() as u64;
    let max_event_seq = persisted.last().map(|row| row.event_seq).unwrap_or(0);
    let min_allowed = max_event_seq.saturating_sub(replay_limit);
    if let Some(requested) = after_event_seq
        && requested < min_allowed
    {
        return Err(ApiError::invalid_argument(
            "replay point is outside retention window",
            json!({ "after_event_seq": requested, "min_allowed": min_allowed }),
        ));
    }

    let floor = after_event_seq.unwrap_or(min_allowed);
    Ok(persisted
        .into_iter()
        .filter(|record| record.event_seq > floor)
        .map(|record| StreamEventEnvelope {
            request_id: record.request_id,
            session_id: record.session_id,
            turn_id: record.turn_id,
            event_seq: record.event_seq,
            turn_seq: record.turn_seq,
            timestamp: record.timestamp,
            event_type: record.event_type,
            stream_source: record.stream_source,
            payload: record.payload,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn list_session_timeline(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventsQuery>,
    Extension(ctx): Extension<crate::middleware::RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let events = state
        .list_session_events(&session_id, query.after_event_seq)?
        .into_iter()
        .map(|record| StreamEventEnvelope {
            request_id: record.request_id,
            session_id: record.session_id,
            turn_id: record.turn_id,
            event_seq: record.event_seq,
            turn_seq: record.turn_seq,
            timestamp: record.timestamp,
            event_type: record.event_type,
            stream_source: record.stream_source,
            payload: record.payload,
        })
        .collect::<Vec<StreamEventEnvelope>>();
    Ok(Json(SessionTimelineResponse {
        request_id: ctx.request_id,
        session_id,
        events,
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
    let (after_event_seq, mut rx) = {
        let last_event_id = headers
            .get("Last-Event-ID")
            .and_then(|value| value.to_str().ok());
        let after_event_seq = resolve_resume_seq(query.after_event_seq, last_event_id);
        let session = session_runtime.lock().await;
        (after_event_seq, session.event_tx.subscribe())
    };
    let replay = replay_events_from_transcript(&state, &session_id, after_event_seq)?;

    let state_for_recovery = state.clone();
    let request_id = headers
        .get("X-Request-Id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("req_stream")
        .to_string();
    if gateway_stream_debug_enabled() {
        info!(
            request_id = %request_id,
            session_id = %session_id,
            after_event_seq = ?query.after_event_seq,
            replay_count = replay.len(),
            "stream_debug.stream_events_opened"
        );
    }
    let event_stream = stream! {
        let mut last_sent_event_seq = replay.last().map(|event| event.event_seq).unwrap_or(0);
        for event in replay {
            last_sent_event_seq = event.event_seq;
            yield Ok(to_sse_event(event));
        }
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if !should_emit_broadcast_event(last_sent_event_seq, event.event_seq) {
                        continue;
                    }
                    last_sent_event_seq = event.event_seq;
                    yield Ok(to_sse_event(event));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let recovered = replay_events_from_transcript(
                        &state_for_recovery,
                        &session_id,
                        Some(last_sent_event_seq),
                    );
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
                                yield Ok(to_sse_event(event));
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
                            yield Ok(to_sse_event(recovery_error));
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    if gateway_stream_debug_enabled() {
                        info!(
                            request_id = %request_id,
                            session_id = %session_id,
                            last_sent_event_seq = last_sent_event_seq,
                            "stream_debug.stream_events_closed"
                        );
                    }
                    break;
                }
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[cfg(test)]
mod tests {
    use super::{resolve_resume_seq, should_emit_broadcast_event};

    #[test]
    fn resolve_resume_seq_prefers_after_event_seq() {
        assert_eq!(resolve_resume_seq(Some(9), Some("3")), Some(9));
    }

    #[test]
    fn resolve_resume_seq_uses_last_event_id_when_query_absent() {
        assert_eq!(resolve_resume_seq(None, Some("7")), Some(7));
    }

    #[test]
    fn should_emit_broadcast_event_filters_replay_overlap_duplicates() {
        assert!(!should_emit_broadcast_event(10, 10));
        assert!(!should_emit_broadcast_event(10, 9));
        assert!(should_emit_broadcast_event(10, 11));
    }
}
