use std::collections::HashSet;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use openjax_core::{ApprovalHandler, ApprovalRequest};
use openjax_gateway::{AppState, build_app};
use serde_json::Value;
use tower::ServiceExt;

fn app_with_api_key(api_key: &str) -> (axum::Router, AppState) {
    let mut keys = HashSet::new();
    keys.insert(api_key.to_string());
    let state = AppState::new_with_api_keys(keys);
    let app = build_app(state.clone());
    (app, state)
}

fn auth_header(api_key: &str) -> String {
    format!("Bearer {}", api_key)
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

#[tokio::test]
async fn create_session_requires_auth() {
    let (app, _state) = app_with_api_key("test-key");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "UNAUTHENTICATED");
}

#[tokio::test]
async fn clear_command_submit_and_polling_flow() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create request"),
        )
        .await
        .expect("create response");
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"].as_str().expect("session_id");

    let submit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}/turns", session_id))
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"input":"/clear"}"#))
                .expect("submit request"),
        )
        .await
        .expect("submit response");
    assert_eq!(submit_response.status(), StatusCode::OK);
    let submit_body = response_json(submit_response).await;
    let turn_id = submit_body["turn_id"].as_str().expect("turn_id");

    let turn_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/turns/{}", session_id, turn_id))
                .header("Authorization", auth_header(api_key))
                .body(Body::empty())
                .expect("turn request"),
        )
        .await
        .expect("turn response");
    assert_eq!(turn_response.status(), StatusCode::OK);
    let turn_body = response_json(turn_response).await;
    assert_eq!(turn_body["status"], "completed");
    assert_eq!(turn_body["assistant_message"], "session cleared");
}

#[tokio::test]
async fn compact_endpoint_returns_not_implemented() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create request"),
        )
        .await
        .expect("create response");
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"].as_str().expect("session_id");

    let compact_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}:compact", session_id))
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"strategy":"default"}"#))
                .expect("compact request"),
        )
        .await
        .expect("compact response");
    assert_eq!(compact_response.status(), StatusCode::NOT_IMPLEMENTED);
    let compact_body = response_json(compact_response).await;
    assert_eq!(compact_body["error"]["code"], "NOT_IMPLEMENTED");
}

#[tokio::test]
async fn approval_resolve_second_call_returns_conflict() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(api_key))
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

    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");
    let approval_handler = {
        let session = session_runtime.lock().await;
        session.approval_handler.clone()
    };
    let approval_id = "approval_test_1".to_string();
    let approval_id_for_task = approval_id.clone();
    let waiter = tokio::spawn(async move {
        approval_handler
            .request_approval(ApprovalRequest {
                request_id: approval_id_for_task,
                target: "cmd".to_string(),
                reason: "test".to_string(),
            })
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    let resolve_uri = format!(
        "/api/v1/sessions/{}/approvals/{}:resolve",
        session_id, approval_id
    );
    let first_resolve = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&resolve_uri)
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"approved":true}"#))
                .expect("first resolve request"),
        )
        .await
        .expect("first resolve response");
    assert_eq!(first_resolve.status(), StatusCode::OK);

    let second_resolve = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&resolve_uri)
                .header("Authorization", auth_header(api_key))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"approved":true}"#))
                .expect("second resolve request"),
        )
        .await
        .expect("second resolve response");
    assert_eq!(second_resolve.status(), StatusCode::CONFLICT);
    let second_body = response_json(second_resolve).await;
    assert_eq!(second_body["error"]["code"], "CONFLICT");

    let waiter_result = waiter.await.expect("waiter task joined");
    assert_eq!(waiter_result.expect("approval resolved"), true);
}

#[tokio::test]
async fn sse_replay_out_of_window_returns_invalid_argument() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(api_key))
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

    let session_runtime = state
        .get_session(&session_id)
        .await
        .expect("session runtime exists");
    {
        let mut session = session_runtime.lock().await;
        for i in 0..1100 {
            let event = session.create_gateway_event(
                "req_test",
                &session_id,
                Some("turn_1".to_string()),
                "assistant_delta",
                serde_json::json!({ "idx": i }),
            );
            session.publish_event(event);
        }
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/sessions/{}/events?after_event_seq=1",
                    session_id
                ))
                .header("Authorization", auth_header(api_key))
                .body(Body::empty())
                .expect("events request"),
        )
        .await
        .expect("events response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
}
