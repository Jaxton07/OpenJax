use std::collections::HashSet;

use openjax_gateway::state::{AppState, StreamEventEnvelope, TurnStatus};
use serde_json::{Value, json};

fn stream_event(
    session_id: &str,
    event_seq: u64,
    turn_seq: u64,
    turn_id: &str,
    event_type: &str,
    payload: Value,
    stream_source: &str,
) -> StreamEventEnvelope {
    StreamEventEnvelope {
        request_id: "req_compat".to_string(),
        session_id: session_id.to_string(),
        turn_id: Some(turn_id.to_string()),
        event_seq,
        turn_seq,
        timestamp: "2026-03-21T00:00:00Z".to_string(),
        event_type: event_type.to_string(),
        stream_source: stream_source.to_string(),
        payload,
    }
}

async fn loaded_turn_state<F>(build_events: F) -> (TurnStatus, Option<String>)
where
    F: FnOnce(&str) -> Vec<StreamEventEnvelope>,
{
    let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
    let session_id = app_state.create_session().await.expect("create session");

    for event in build_events(&session_id) {
        app_state.append_event(&event).expect("append event");
    }

    app_state.remove_session(&session_id).await;
    let session = app_state
        .get_session(&session_id)
        .await
        .expect("reload session");
    let guard = session.lock().await;
    let turn = guard.turns.get("turn_1").expect("turn exists");
    (turn.status, turn.assistant_message.clone())
}

#[tokio::test]
async fn assistant_message_alone_does_not_finalize_turn() {
    let (status, assistant_message) = loaded_turn_state(|session_id| {
        vec![
            stream_event(
                session_id,
                1,
                1,
                "turn_1",
                "turn_started",
                json!({}),
                "synthetic",
            ),
            stream_event(
                session_id,
                2,
                2,
                "turn_1",
                "assistant_message",
                json!({ "content": "legacy" }),
                "synthetic",
            ),
        ]
    })
    .await;

    assert_eq!(status, TurnStatus::Running);
    assert!(assistant_message.is_none());
}

#[tokio::test]
async fn response_completed_overrides_legacy_assistant_message() {
    let (status, assistant_message) = loaded_turn_state(|session_id| {
        vec![
            stream_event(
                session_id,
                1,
                1,
                "turn_1",
                "turn_started",
                json!({}),
                "synthetic",
            ),
            stream_event(
                session_id,
                2,
                2,
                "turn_1",
                "assistant_message",
                json!({ "content": "legacy" }),
                "synthetic",
            ),
            stream_event(
                session_id,
                3,
                3,
                "turn_1",
                "response_completed",
                json!({ "content": "final" }),
                "model_live",
            ),
        ]
    })
    .await;

    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(assistant_message.as_deref(), Some("final"));
}

#[tokio::test]
async fn response_completed_remains_authoritative_after_later_assistant_message() {
    let (status, assistant_message) = loaded_turn_state(|session_id| {
        vec![
            stream_event(
                session_id,
                1,
                1,
                "turn_1",
                "turn_started",
                json!({}),
                "synthetic",
            ),
            stream_event(
                session_id,
                2,
                2,
                "turn_1",
                "response_completed",
                json!({ "content": "final" }),
                "model_live",
            ),
            stream_event(
                session_id,
                3,
                3,
                "turn_1",
                "assistant_message",
                json!({ "content": "legacy" }),
                "synthetic",
            ),
        ]
    })
    .await;

    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(assistant_message.as_deref(), Some("final"));
}
