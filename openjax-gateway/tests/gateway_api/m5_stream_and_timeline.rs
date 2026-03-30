use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use openjax_gateway::AppState;
use openjax_gateway::state::{
    StreamEventEnvelope, TurnRuntime, TurnStatus, append_then_publish,
    handle_key_event_append_failure,
};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::TryRecvError;
use tokio::time::{Duration, timeout};
use tower::ServiceExt;

use crate::gateway_api::helpers::{
    app_with_api_key, auth_header, create_session_for_test, login, response_json,
};

fn seed_event(
    session_id: &str,
    event_seq: u64,
    turn_seq: u64,
    turn_id: Option<&str>,
    event_type: &str,
    payload: Value,
) -> StreamEventEnvelope {
    StreamEventEnvelope {
        request_id: "req_seed".to_string(),
        session_id: session_id.to_string(),
        turn_id: turn_id.map(ToString::to_string),
        event_seq,
        turn_seq,
        timestamp: "2026-03-30T00:00:00Z".to_string(),
        event_type: event_type.to_string(),
        stream_source: "synthetic".to_string(),
        payload,
    }
}

async fn publish_1100_response_text_delta_events(state: &AppState, session_id: &str) {
    let session_runtime = state
        .get_session(session_id)
        .await
        .expect("session runtime exists");
    let mut session = session_runtime.lock().await;
    for i in 0..1100 {
        let event = session.create_gateway_event(
            "req_test",
            session_id,
            Some("turn_1".to_string()),
            "response_text_delta",
            serde_json::json!({ "idx": i }),
            None,
        );
        append_then_publish(state, &mut session, event).expect("append+publish event");
    }
}

async fn read_sse_event_data(
    response: axum::response::Response,
    expected_count: usize,
) -> Vec<Value> {
    let mut body_stream = response.into_body().into_data_stream();
    let mut buffer = String::new();
    let mut parsed = Vec::new();

    while parsed.len() < expected_count {
        let next = timeout(Duration::from_secs(2), body_stream.next())
            .await
            .expect("read sse chunk timeout");
        let Some(chunk) = next else {
            break;
        };
        let bytes = chunk.expect("read sse bytes");
        let text = String::from_utf8(bytes.to_vec()).expect("sse chunk utf8");
        buffer.push_str(&text.replace("\r\n", "\n"));

        while let Some(split_idx) = buffer.find("\n\n") {
            let frame = buffer[..split_idx].to_string();
            buffer = buffer[split_idx + 2..].to_string();

            let data_lines = frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>();
            if data_lines.is_empty() {
                continue;
            }
            let data = data_lines.join("\n");
            parsed.push(serde_json::from_str::<Value>(&data).expect("sse json payload"));
            if parsed.len() >= expected_count {
                break;
            }
        }
    }

    parsed
}

#[tokio::test]
async fn sse_replay_out_of_window_returns_invalid_argument() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create request"),
        )
        .await
        .expect("create response");
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"]
        .as_str()
        .expect("session_id")
        .to_string();

    publish_1100_response_text_delta_events(&state, &session_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/sessions/{}/events?after_event_seq=1",
                    session_id
                ))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(body["error"]["details"]["min_allowed"], 76);
}

#[tokio::test]
async fn sse_resume_query_takes_precedence_over_last_event_id() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create request"),
        )
        .await
        .expect("create response");
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"]
        .as_str()
        .expect("session_id")
        .to_string();

    publish_1100_response_text_delta_events(&state, &session_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/sessions/{}/events?after_event_seq=1",
                    session_id
                ))
                .header("Authorization", auth_header(&access_token))
                .header("Last-Event-ID", "1099")
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn append_failure_does_not_emit_sse_event() {
    let state = AppState::new_with_api_keys_for_test(Default::default());
    let session_id = state.create_session().await.expect("create session");
    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");

    let mut rx = {
        let session = session_runtime.lock().await;
        session.event_tx.subscribe()
    };

    let persisted = seed_event(
        &session_id,
        1,
        0,
        None,
        "seed_event",
        json!({ "seed": true }),
    );
    state
        .append_event(&persisted)
        .expect("persist seed event to force duplicate seq");

    let mut session = session_runtime.lock().await;
    let event = session.create_gateway_event(
        "req_test",
        &session_id,
        None,
        "session_shutdown",
        json!({ "reason": "test" }),
        Some("synthetic"),
    );
    let err = append_then_publish(&state, &mut session, event).expect_err("append should fail");
    assert_eq!(err.code, "INTERNAL");

    assert!(matches!(
        rx.try_recv(),
        Err(TryRecvError::Empty) | Err(TryRecvError::Closed)
    ));
}

#[tokio::test]
async fn sse_and_timeline_are_identical_for_same_turn() {
    let state = AppState::new_with_api_keys_for_test(Default::default());
    let session_id = state.create_session().await.expect("create session");
    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");

    let mut rx = {
        let session = session_runtime.lock().await;
        session.event_tx.subscribe()
    };

    {
        let mut session = session_runtime.lock().await;
        for idx in 0..3 {
            let event = session.create_gateway_event(
                "req_test",
                &session_id,
                Some("turn_1".to_string()),
                "response_text_delta",
                json!({ "idx": idx }),
                Some("model_live"),
            );
            append_then_publish(&state, &mut session, event).expect("append+publish");
        }
    }

    let mut streamed = Vec::new();
    for _ in 0..3 {
        let event = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("recv timeout")
            .expect("recv event");
        streamed.push(event);
    }

    let persisted = state
        .list_session_events(&session_id, None)
        .expect("list persisted events");
    assert_eq!(streamed.len(), persisted.len());
    for (live, saved) in streamed.iter().zip(persisted.iter()) {
        assert_eq!(live.event_seq, saved.event_seq);
        assert_eq!(live.turn_seq, saved.turn_seq);
        assert_eq!(live.event_type, saved.event_type);
        assert_eq!(live.turn_id.as_deref(), saved.turn_id.as_deref());
        assert_eq!(
            live.payload,
            serde_json::from_str::<Value>(&saved.payload_json).expect("parse payload")
        );
    }
}

#[tokio::test]
async fn key_event_append_failure_marks_turn_failed_and_emits_single_transcript_append_error() {
    let state = AppState::new_with_api_keys_for_test(Default::default());
    let session_id = state.create_session().await.expect("create session");
    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");

    let mut rx = {
        let session = session_runtime.lock().await;
        session.event_tx.subscribe()
    };

    state
        .append_event(&seed_event(
            &session_id,
            1,
            1,
            Some("turn_1"),
            "seed_conflict",
            json!({}),
        ))
        .expect("seed conflict seq");

    let mut session = session_runtime.lock().await;
    session
        .turns
        .insert("turn_1".to_string(), TurnRuntime::queued());
    let key_event = session.create_gateway_event(
        "req_test",
        &session_id,
        Some("turn_1".to_string()),
        "response_text_delta",
        json!({ "content_delta": "hello" }),
        Some("model_live"),
    );
    let key_error = append_then_publish(&state, &mut session, key_event.clone())
        .expect_err("key event append fails");
    handle_key_event_append_failure(&state, &mut session, &key_event, key_error)
        .expect("emit single append-failure response_error");

    let turn = session.turns.get("turn_1").expect("turn exists");
    assert_eq!(turn.status, TurnStatus::Failed);
    let turn_error = turn.error.as_ref().expect("turn error");
    assert_eq!(turn_error.code, "TRANSCRIPT_APPEND_FAILED");

    let emitted = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("recv timeout")
        .expect("recv emitted error");
    assert_eq!(emitted.event_type, "response_error");
    assert_eq!(emitted.payload["code"], "TRANSCRIPT_APPEND_FAILED");
    assert!(matches!(
        rx.try_recv(),
        Err(TryRecvError::Empty) | Err(TryRecvError::Closed)
    ));
}

#[tokio::test]
async fn when_error_event_append_also_fails_turn_stops_without_recursive_error_emit() {
    let state = AppState::new_with_api_keys_for_test(Default::default());
    let session_id = state.create_session().await.expect("create session");
    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");

    let mut rx = {
        let session = session_runtime.lock().await;
        session.event_tx.subscribe()
    };

    state
        .append_event(&seed_event(
            &session_id,
            1,
            1,
            Some("turn_1"),
            "seed_conflict_key",
            json!({}),
        ))
        .expect("seed key conflict");
    state
        .append_event(&seed_event(
            &session_id,
            2,
            2,
            Some("turn_1"),
            "seed_conflict_error",
            json!({}),
        ))
        .expect("seed error conflict");

    let mut session = session_runtime.lock().await;
    session
        .turns
        .insert("turn_1".to_string(), TurnRuntime::queued());
    let key_event = session.create_gateway_event(
        "req_test",
        &session_id,
        Some("turn_1".to_string()),
        "response_completed",
        json!({ "content": "done" }),
        Some("model_live"),
    );
    let key_error = append_then_publish(&state, &mut session, key_event.clone())
        .expect_err("key event append fails");
    let err = handle_key_event_append_failure(&state, &mut session, &key_event, key_error)
        .expect_err("error event append also fails");
    assert_eq!(err.code, "INTERNAL");
    assert_eq!(session.next_event_seq, 3);
    let turn = session.turns.get("turn_1").expect("turn exists");
    assert_eq!(turn.status, TurnStatus::Failed);
    assert!(matches!(
        rx.try_recv(),
        Err(TryRecvError::Empty) | Err(TryRecvError::Closed)
    ));
}

#[tokio::test]
async fn last_event_id_resume_replays_exact_missing_events_from_transcript() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    {
        let session_runtime = state
            .get_session(&session_id)
            .await
            .expect("session runtime exists");
        let mut session = session_runtime.lock().await;
        for idx in 0..5 {
            let event = session.create_gateway_event(
                "req_test",
                &session_id,
                Some("turn_1".to_string()),
                "response_text_delta",
                json!({ "idx": idx }),
                Some("model_live"),
            );
            append_then_publish(&state, &mut session, event).expect("append+publish");
        }
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/events", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Last-Event-ID", "2")
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");
    assert_eq!(response.status(), StatusCode::OK);

    let replayed = read_sse_event_data(response, 3).await;
    let replayed_seq = replayed
        .iter()
        .map(|event| event["event_seq"].as_u64().expect("event seq"))
        .collect::<Vec<_>>();
    assert_eq!(replayed_seq, vec![3, 4, 5]);

    let persisted = state
        .list_session_events(&session_id, Some(2))
        .expect("list persisted events");
    let persisted_seq = persisted
        .iter()
        .map(|event| event.event_seq)
        .collect::<Vec<_>>();
    assert_eq!(replayed_seq, persisted_seq);
}

#[tokio::test]
async fn session_messages_endpoint_returns_persisted_messages() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    // Submit a regular turn (not /clear via slash) to create messages
    let submit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}/turns", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"input":"hello"}"#))
                .expect("submit request"),
        )
        .await
        .expect("submit response");
    assert_eq!(submit_response.status(), StatusCode::OK);

    let messages_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/messages", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("messages request"),
        )
        .await
        .expect("messages response");
    assert_eq!(messages_response.status(), StatusCode::OK);
    let messages_body = response_json(messages_response).await;
    // Only user message is created with submit_turn (no LLM for assistant response)
    assert_eq!(messages_body["messages"][0]["role"], "user");
}

#[tokio::test]
async fn session_timeline_endpoint_returns_persisted_events() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    // Submit a regular turn (not /clear via slash) to create events
    let submit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}/turns", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"input":"hello"}"#))
                .expect("submit request"),
        )
        .await
        .expect("submit response");
    assert_eq!(submit_response.status(), StatusCode::OK);

    let timeline_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/timeline", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("timeline request"),
        )
        .await
        .expect("timeline response");
    assert_eq!(timeline_response.status(), StatusCode::OK);
    let timeline_body = response_json(timeline_response).await;
    let events = timeline_body["events"].as_array().expect("events");
    // Only user_message event is created with submit_turn (no LLM for response_completed)
    assert!(!events.is_empty());
    assert_eq!(events[0]["type"], "user_message");
}
