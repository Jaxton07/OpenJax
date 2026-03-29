use axum::body::Body;
use axum::http::{Request, StatusCode};
use openjax_gateway::AppState;
use tower::ServiceExt;

use crate::gateway_api::helpers::{
    app_with_api_key, auth_header, create_session_for_test, login, response_json,
};

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
        session.publish_event(event);
    }
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
