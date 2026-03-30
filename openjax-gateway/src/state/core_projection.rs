//! Core event -> gateway runtime projection.

use openjax_protocol::Event;
use serde_json::Value;
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::error::ApiError;
use crate::event_mapper::{MapResult, map_core_event_payload_result};

use super::events::AppState;
use super::runtime::{
    ApiTurnError, SessionRuntime, TurnRuntime, TurnStatus, gateway_stream_debug_enabled,
    log_preview, reasoning_preview,
};
use super::{append_then_publish, handle_key_event_append_failure};

const AFTER_DISPATCH_LOG_TARGET: &str = "openjax_after_dispatcher";
const AFTER_DISPATCH_PREFIX: &str = "OPENJAX_AFTER_DISPATCH";

pub(crate) fn apply_turn_runtime_event(turn: &mut TurnRuntime, event_type: &str, payload: &Value) {
    if event_type == "turn_started" {
        turn.status = TurnStatus::Running;
        return;
    }
    if event_type == "turn_completed" {
        if !matches!(turn.status, TurnStatus::Failed) {
            turn.status = TurnStatus::Completed;
        }
        return;
    }
    if event_type == "response_error" || event_type == "error" {
        turn.status = TurnStatus::Failed;
        turn.error = Some(ApiTurnError {
            code: payload
                .get("code")
                .and_then(|value| value.as_str())
                .unwrap_or("UPSTREAM_ERROR")
                .to_string(),
            message: payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("response failed")
                .to_string(),
            retryable: payload
                .get("retryable")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            details: payload.clone(),
        });
        return;
    }
    if event_type == "turn_interrupted" {
        turn.status = TurnStatus::Failed;
        turn.error = Some(ApiTurnError {
            code: "TURN_ABORTED".to_string(),
            message: "turn aborted by user".to_string(),
            retryable: false,
            details: payload.clone(),
        });
        return;
    }
    if event_type == "assistant_message" {
        // Intentional decommission rule:
        // assistant_message-only history is no longer authoritative and does not
        // populate turn.assistant_message in runtime replay.
        return;
    }
    if event_type == "response_completed"
        && let Some(content) = payload.get("content").and_then(|value| value.as_str())
    {
        turn.assistant_message = Some(content.to_string());
        turn.status = TurnStatus::Completed;
    }
}

pub(crate) fn map_core_event(
    app_state: &AppState,
    session: &mut SessionRuntime,
    session_id: &str,
    request_id: &str,
    event: Event,
    turn_id_tx: &mut Option<oneshot::Sender<Result<String, ApiError>>>,
) -> Option<String> {
    let mapping = match map_core_event_payload_result(&event) {
        MapResult::Mapped(mapping) => mapping,
        MapResult::IgnoredInternal => return None,
        MapResult::Unmapped(variant) => {
            warn!(
                core_event_variant = variant,
                "unmapped core event hit gateway coverage gate"
            );
            debug_assert!(
                false,
                "unmapped core event variant hit gateway mapping gate: {variant}"
            );
            return None;
        }
    };
    let core_turn_id = mapping.core_turn_id;
    let event_type = mapping.event_type;
    let payload = mapping.payload;
    let stream_source = mapping.stream_source;

    let public_turn_id = core_turn_id.map(|tid| session.get_or_create_public_turn_id(tid));
    if let Some(turn_id) = &public_turn_id {
        let turn = session
            .turns
            .entry(turn_id.clone())
            .or_insert_with(TurnRuntime::queued);
        apply_turn_runtime_event(turn, event_type, &payload);
        if event_type == "response_completed"
            && let Some(content) = payload.get("content").and_then(|value| value.as_str())
        {
            turn.assistant_message = Some(content.to_string());
            let _ = app_state.append_message(session_id, Some(turn_id), "assistant", content);
        }
    }

    if let Some(turn_id) = public_turn_id.clone()
        && let Some(tx) = turn_id_tx.take()
    {
        let _ = tx.send(Ok(turn_id));
    }

    let envelope = session.create_gateway_event(
        request_id,
        session_id,
        public_turn_id.clone(),
        event_type,
        payload,
        stream_source,
    );
    if event_type == "reasoning_delta" {
        let delta_raw = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let (delta_preview, delta_preview_truncated) = reasoning_preview(delta_raw);
        info!(
            target: AFTER_DISPATCH_LOG_TARGET,
            session_id = %session_id,
            turn_id = ?public_turn_id,
            flow_prefix = AFTER_DISPATCH_PREFIX,
            flow_node = "gateway.reasoning.publish",
            flow_route = "reasoning_delta",
            flow_next = "frontend.reasoning_delta",
            event_seq = envelope.event_seq,
            turn_seq = envelope.turn_seq,
            delta_len = delta_raw.chars().count(),
            delta_preview = %delta_preview,
            delta_preview_truncated = delta_preview_truncated,
            "after_dispatcher_trace"
        );
    }
    if gateway_stream_debug_enabled()
        && matches!(
            event_type,
            "response_started"
                | "response_text_delta"
                | "response_completed"
                | "turn_completed"
                | "response_error"
        )
    {
        let delta_raw = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str());
        let content_raw = envelope.payload.get("content").and_then(|value| value.as_str());
        let delta_len = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let content_len = envelope
            .payload
            .get("content")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let (delta_preview, delta_preview_truncated) = delta_raw
            .map(|value| log_preview(value, 24))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let (content_preview, content_preview_truncated) = content_raw
            .map(|value| log_preview(value, 80))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let assistant_len = public_turn_id
            .as_ref()
            .and_then(|turn_id| session.turns.get(turn_id))
            .and_then(|turn| turn.assistant_message.as_ref())
            .map(|value| value.len());
        let event_gap_ms = session
            .get_last_event_emitted_at()
            .map(|ts: std::time::Instant| ts.elapsed().as_millis() as u64);
        info!(
            session_id = %session_id,
            turn_id = ?public_turn_id,
            event_type = event_type,
            event_seq = envelope.event_seq,
            turn_seq = envelope.turn_seq,
            stream_source = %envelope.stream_source,
            delta_len = ?delta_len,
            delta_preview = ?delta_preview,
            delta_preview_truncated = ?delta_preview_truncated,
            content_len = ?content_len,
            content_preview = ?content_preview,
            content_preview_truncated = ?content_preview_truncated,
            assistant_message_len = ?assistant_len,
            event_gap_ms = ?event_gap_ms,
            "stream_debug.gateway_event_emitted"
        );
    }
    if (event_type == "tool_call_started"
        || event_type == "tool_call_ready"
        || event_type == "tool_call_completed")
        && envelope.payload.get("tool_call_id").is_some()
    {
        info!(
            event_type = event_type,
            turn_id = ?public_turn_id,
            tool_call_id = ?envelope.payload.get("tool_call_id").and_then(|v| v.as_str()),
            "tool event mapped"
        );
    }
    let should_emit_append_failure_error =
        envelope.turn_id.is_some() && envelope.event_type != "response_error";
    if let Err(err) = append_then_publish(app_state, session, envelope.clone()) {
        warn!(
            session_id = %envelope.session_id,
            event_seq = envelope.event_seq,
            event_type = %envelope.event_type,
            error = %err.message,
            "failed to append core mapped event before publish"
        );
        if should_emit_append_failure_error
            && let Err(error_event_err) =
                handle_key_event_append_failure(app_state, session, &envelope, err)
        {
            warn!(
                session_id = %envelope.session_id,
                event_type = %envelope.event_type,
                turn_id = ?envelope.turn_id,
                error = %error_event_err.message,
                "failed to emit transcript append failure response_error"
            );
        }
    }

    public_turn_id
}

pub fn core_event_mapping_gate(event: &Event) -> Result<(), &'static str> {
    match map_core_event_payload_result(event) {
        MapResult::Mapped(_) | MapResult::IgnoredInternal => Ok(()),
        MapResult::Unmapped(variant) => Err(variant),
    }
}

pub(crate) fn first_turn_id(events: &[Event]) -> Option<u64> {
    for event in events {
        match event {
            Event::TurnStarted { turn_id }
            | Event::ToolCallStarted { turn_id, .. }
            | Event::ToolCallCompleted { turn_id, .. }
            | Event::ToolCallArgsDelta { turn_id, .. }
            | Event::ToolCallReady { turn_id, .. }
            | Event::ToolCallProgress { turn_id, .. }
            | Event::ToolCallFailed { turn_id, .. }
            | Event::AssistantMessage { turn_id, .. }
            | Event::ResponseStarted { turn_id, .. }
            | Event::ResponseTextDelta { turn_id, .. }
            | Event::ReasoningDelta { turn_id, .. }
            | Event::ToolCallsProposed { turn_id, .. }
            | Event::ToolBatchCompleted { turn_id, .. }
            | Event::ResponseResumed { turn_id, .. }
            | Event::ResponseCompleted { turn_id, .. }
            | Event::ResponseError { turn_id, .. }
            | Event::ApprovalRequested { turn_id, .. }
            | Event::ApprovalResolved { turn_id, .. }
            | Event::LoopWarning { turn_id, .. }
            | Event::TurnCompleted { turn_id } => return Some(*turn_id),
            Event::AgentSpawned { .. }
            | Event::AgentStatusChanged { .. }
            | Event::ContextUsageUpdated { .. }
            | Event::ContextCompacted { .. }
            | Event::ShutdownComplete => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use openjax_protocol::ShellExecutionMetadata;
    use openjax_store::SessionRepository;
    use serde_json::json;
    use std::collections::HashSet;

    fn seed_session(app_state: &AppState, session_id: &str) {
        app_state
            .store
            .create_session(session_id, None)
            .expect("seed test session");
    }

    #[test]
    fn turn_status_remains_failed_after_turn_completed() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        seed_session(&app_state, "sess_1");
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseError {
                turn_id: 1,
                code: "ERR".to_string(),
                message: "failed".to_string(),
                retryable: false,
            },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnCompleted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.status, TurnStatus::Failed);
    }

    #[test]
    fn turn_message_only_updates_from_response_completed() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        seed_session(&app_state, "sess_1");
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnStarted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::AssistantMessage {
                turn_id: 1,
                content: "legacy".to_string(),
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert!(turn.assistant_message.is_none());

        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseCompleted {
                turn_id: 1,
                content: "final".to_string(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.assistant_message.as_deref(), Some("final"));
    }

    #[test]
    fn response_completed_overrides_later_assistant_message() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        seed_session(&app_state, "sess_1");
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;

        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnStarted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseCompleted {
                turn_id: 1,
                content: "final".to_string(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::AssistantMessage {
                turn_id: 1,
                content: "legacy".to_string(),
            },
            &mut turn_id_tx,
        );

        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.status, TurnStatus::Completed);
        assert_eq!(turn.assistant_message.as_deref(), Some("final"));
    }

    #[test]
    fn first_turn_id_supports_reasoning_delta() {
        let turn_id = first_turn_id(&[Event::ReasoningDelta {
            turn_id: 7,
            content_delta: "thinking".to_string(),
            stream_source: openjax_protocol::StreamSource::ModelLive,
        }]);
        assert_eq!(turn_id, Some(7));
    }

    #[test]
    fn turn_interrupted_marks_turn_failed_with_abort_error() {
        let mut turn = TurnRuntime::queued();
        apply_turn_runtime_event(
            &mut turn,
            "turn_interrupted",
            &json!({ "reason": "user_abort" }),
        );
        assert_eq!(turn.status, TurnStatus::Failed);
        let error = turn.error.expect("abort error");
        assert_eq!(error.code, "TURN_ABORTED");
        assert!(!error.retryable);
    }

    #[test]
    fn tool_call_completed_replay_event_keeps_shell_metadata() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        seed_session(&app_state, "sess_1");
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;

        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ToolCallCompleted {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                ok: true,
                output: "done".to_string(),
                shell_metadata: Some(ShellExecutionMetadata {
                    result_class: "success".to_string(),
                    backend: "sandbox".to_string(),
                    exit_code: 0,
                    policy_decision: "allow".to_string(),
                    runtime_allowed: true,
                    degrade_reason: None,
                    runtime_deny_reason: None,
                }),
                display_name: Some("Run Shell".to_string()),
            },
            &mut turn_id_tx,
        );

        let replay = session
            .replay_from(None)
            .expect("replay events should exist");
        let event = replay.last().expect("tool call event");
        assert_eq!(event.event_type, "tool_call_completed");
        assert_eq!(event.payload["tool_call_id"], "call_1");
        assert_eq!(event.payload["display_name"], "Run Shell");
        assert_eq!(event.payload["shell_metadata"]["backend"], "sandbox");
    }
}
