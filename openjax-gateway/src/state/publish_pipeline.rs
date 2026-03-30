//! Unified append-then-publish pipeline for gateway events.

use serde_json::json;
use tracing::warn;

use crate::error::ApiError;

use super::{AppState, SessionRuntime, StreamEventEnvelope, TurnRuntime, TurnStatus};

pub const TRANSCRIPT_APPEND_FAILED_CODE: &str = "TRANSCRIPT_APPEND_FAILED";

pub fn append_then_publish(
    app_state: &AppState,
    session: &mut SessionRuntime,
    event: StreamEventEnvelope,
) -> Result<(), ApiError> {
    app_state.append_event(&event)?;
    session.publish_event(event);
    session.set_last_event_emitted_at(Some(std::time::Instant::now()));
    Ok(())
}

pub fn handle_key_event_append_failure(
    app_state: &AppState,
    session: &mut SessionRuntime,
    key_event: &StreamEventEnvelope,
    append_error: ApiError,
) -> Result<(), ApiError> {
    if let Some(turn_id) = key_event.turn_id.as_ref() {
        let turn = session
            .turns
            .entry(turn_id.clone())
            .or_insert_with(TurnRuntime::queued);
        turn.status = TurnStatus::Failed;
        turn.error = Some(super::ApiTurnError {
            code: TRANSCRIPT_APPEND_FAILED_CODE.to_string(),
            message: "failed to append event to transcript".to_string(),
            retryable: append_error.retryable,
            details: json!({
                "failed_event_type": key_event.event_type,
                "failed_event_seq": key_event.event_seq,
                "failed_turn_seq": key_event.turn_seq,
                "append_error_code": append_error.code,
                "append_error_message": append_error.message,
                "append_error_details": append_error.details,
            }),
        });
    }

    let error_event = session.create_gateway_event(
        &key_event.request_id,
        &key_event.session_id,
        key_event.turn_id.clone(),
        "response_error",
        json!({
            "code": TRANSCRIPT_APPEND_FAILED_CODE,
            "message": "failed to append event to transcript",
            "retryable": append_error.retryable,
            "failed_event_type": key_event.event_type,
            "failed_event_seq": key_event.event_seq,
            "append_error_code": append_error.code,
        }),
        Some("synthetic"),
    );

    match append_then_publish(app_state, session, error_event) {
        Ok(()) => Ok(()),
        Err(error_append_error) => {
            warn!(
                session_id = %key_event.session_id,
                turn_id = ?key_event.turn_id,
                failed_event_type = %key_event.event_type,
                failed_event_seq = key_event.event_seq,
                error_code = error_append_error.code,
                error_message = %error_append_error.message,
                "failed to append transcript-append-failure response_error; stopping without recursion"
            );
            Err(error_append_error)
        }
    }
}
