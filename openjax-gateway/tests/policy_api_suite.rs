use std::collections::HashSet;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use openjax_gateway::{AppState, build_app};
use serde_json::Value;
use tokio::time::sleep;
use tower::ServiceExt;

fn app_with_api_key(api_key: &str) -> axum::Router {
    let mut keys = HashSet::new();
    keys.insert(api_key.to_string());
    let state = AppState::new_with_api_keys_for_test(keys);
    build_app(state, None)
}

fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

async fn login(app: &axum::Router, owner_key: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Authorization", auth_header(owner_key))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("login request"),
        )
        .await
        .expect("login response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    body["access_token"]
        .as_str()
        .expect("access token")
        .to_string()
}

async fn create_session(app: &axum::Router, access_token: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create session request"),
        )
        .await
        .expect("create session response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    body["session_id"].as_str().expect("session id").to_string()
}

async fn submit_turn(
    app: &axum::Router,
    access_token: &str,
    session_id: &str,
    input: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}/turns", session_id))
                .header("Authorization", auth_header(access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "input": input }).to_string(),
                ))
                .expect("submit turn request"),
        )
        .await
        .expect("submit turn response");
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn session_timeline_events(
    app: &axum::Router,
    access_token: &str,
    session_id: &str,
) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/timeline", session_id))
                .header("Authorization", auth_header(access_token))
                .body(Body::empty())
                .expect("timeline request"),
        )
        .await
        .expect("timeline response");
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await["events"]
        .as_array()
        .expect("events array")
        .clone()
}

#[tokio::test]
async fn publish_returns_incremented_policy_version() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/publish")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("first publish request"),
        )
        .await
        .expect("first publish response");
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = response_json(first).await;
    let first_version = first_body["policy_version"]
        .as_u64()
        .expect("first policy_version");
    assert!(first_version >= 2);

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/publish")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("second publish request"),
        )
        .await
        .expect("second publish response");
    assert_eq!(second.status(), StatusCode::OK);
    let second_body = response_json(second).await;
    let second_version = second_body["policy_version"]
        .as_u64()
        .expect("second policy_version");
    assert_eq!(second_version, first_version + 1);
}

#[tokio::test]
async fn policy_rules_crud_roundtrip() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "deny_exec",
                        "decision": "deny",
                        "priority": 100,
                        "tool_name": "exec_command",
                        "action": "exec",
                        "reason": "block command execution by default"
                    })
                    .to_string(),
                ))
                .expect("create rule request"),
        )
        .await
        .expect("create rule response");
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = response_json(create).await;
    assert_eq!(create_body["rule"]["id"], "deny_exec");
    assert_eq!(create_body["rule"]["decision"], "deny");

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list rules request"),
        )
        .await
        .expect("list rules response");
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = response_json(list).await;
    assert_eq!(list_body["rules"].as_array().expect("rules array").len(), 1);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/deny_exec")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "ask",
                        "priority": 80,
                        "tool_name": "exec_command",
                        "action": "exec",
                        "reason": "ask before execution"
                    })
                    .to_string(),
                ))
                .expect("update rule request"),
        )
        .await
        .expect("update rule response");
    assert_eq!(update.status(), StatusCode::OK);
    let update_body = response_json(update).await;
    assert_eq!(update_body["rule"]["decision"], "ask");

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/policy/rules/deny_exec")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete rule request"),
        )
        .await
        .expect("delete rule response");
    assert_eq!(delete.status(), StatusCode::OK);
    let delete_body = response_json(delete).await;
    assert_eq!(delete_body["status"], "deleted");

    let list_after_delete = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list rules request"),
        )
        .await
        .expect("list rules response");
    assert_eq!(list_after_delete.status(), StatusCode::OK);
    let list_after_delete_body = response_json(list_after_delete).await;
    assert_eq!(
        list_after_delete_body["rules"]
            .as_array()
            .expect("rules array")
            .len(),
        0
    );
}

#[tokio::test]
async fn session_overlay_set_and_clear() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;
    let session_id = create_session(&app, &access_token).await;

    let set_overlay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{}/policy-overlay", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "rules": [
                            {
                                "id": "overlay_deny_exec",
                                "decision": "deny",
                                "priority": 120,
                                "tool_name": "exec_command",
                                "action": "exec",
                                "reason": "deny this session"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("set overlay request"),
        )
        .await
        .expect("set overlay response");
    assert_eq!(set_overlay.status(), StatusCode::OK);
    let set_body = response_json(set_overlay).await;
    assert_eq!(set_body["status"], "set");
    assert_eq!(set_body["rule_count"], 1);
    let set_version = set_body["policy_version"]
        .as_u64()
        .expect("set policy version");

    let clear_overlay = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{}/policy-overlay", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("clear overlay request"),
        )
        .await
        .expect("clear overlay response");
    assert_eq!(clear_overlay.status(), StatusCode::OK);
    let clear_body = response_json(clear_overlay).await;
    assert_eq!(clear_body["status"], "cleared");
    assert_eq!(clear_body["rule_count"], 0);
    let clear_version = clear_body["policy_version"]
        .as_u64()
        .expect("clear policy version");
    assert_eq!(clear_version, set_version + 1);
}

#[tokio::test]
async fn policy_rule_create_update_publish_affects_submit_turn() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;
    let session_id = create_session(&app, &access_token).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "read_file_gate",
                        "decision": "deny",
                        "priority": 120,
                        "tool_name": "read_file",
                        "action": "read",
                        "reason": "deny read before review"
                    })
                    .to_string(),
                ))
                .expect("create rule request"),
        )
        .await
        .expect("create rule response");
    assert_eq!(create.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/read_file_gate")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "ask",
                        "priority": 140,
                        "tool_name": "read_file",
                        "action": "read",
                        "reason": "ask before reading file"
                    })
                    .to_string(),
                ))
                .expect("update rule request"),
        )
        .await
        .expect("update rule response");
    assert_eq!(update.status(), StatusCode::OK);

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/publish")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("publish request"),
        )
        .await
        .expect("publish response");
    assert_eq!(publish.status(), StatusCode::OK);
    let publish_body = response_json(publish).await;
    let published_version = publish_body["policy_version"]
        .as_u64()
        .expect("published policy_version");

    let submit_body = submit_turn(
        &app,
        &access_token,
        &session_id,
        "tool:read_file path=Cargo.toml",
    )
    .await;
    assert!(submit_body["turn_id"].as_str().is_some());

    let mut approval_event: Option<Value> = None;
    for _ in 0..30 {
        let events = session_timeline_events(&app, &access_token, &session_id).await;
        if let Some(found) = events
            .iter()
            .find(|event| event["type"] == "approval_requested")
        {
            approval_event = Some(found.clone());
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    let approval_event = approval_event.expect("approval_requested event should be persisted");
    assert_eq!(
        approval_event["payload"]["matched_rule_id"],
        Value::String("read_file_gate".to_string())
    );
    assert_eq!(
        approval_event["payload"]["policy_version"],
        Value::Number(published_version.into())
    );
}

#[tokio::test]
async fn create_policy_rule_rejects_blank_id_or_reason() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let blank_id = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "   ",
                        "decision": "deny",
                        "reason": "block"
                    })
                    .to_string(),
                ))
                .expect("blank id request"),
        )
        .await
        .expect("blank id response");
    assert_eq!(blank_id.status(), StatusCode::BAD_REQUEST);
    let blank_id_body = response_json(blank_id).await;
    assert_eq!(blank_id_body["error"]["code"], "INVALID_ARGUMENT");

    let blank_reason = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "deny_exec_blank_reason",
                        "decision": "deny",
                        "reason": "   "
                    })
                    .to_string(),
                ))
                .expect("blank reason request"),
        )
        .await
        .expect("blank reason response");
    assert_eq!(blank_reason.status(), StatusCode::BAD_REQUEST);
    let blank_reason_body = response_json(blank_reason).await;
    assert_eq!(blank_reason_body["error"]["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn create_policy_rule_rejects_duplicate_rule_id() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let create_once = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "duplicate_rule",
                        "decision": "ask",
                        "reason": "first create"
                    })
                    .to_string(),
                ))
                .expect("first create request"),
        )
        .await
        .expect("first create response");
    assert_eq!(create_once.status(), StatusCode::OK);

    let create_twice = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "duplicate_rule",
                        "decision": "deny",
                        "reason": "second create"
                    })
                    .to_string(),
                ))
                .expect("second create request"),
        )
        .await
        .expect("second create response");
    assert_eq!(create_twice.status(), StatusCode::CONFLICT);
    let body = response_json(create_twice).await;
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn update_and_delete_nonexistent_policy_rule_return_not_found() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let update_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/missing_rule")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "deny",
                        "reason": "no such rule"
                    })
                    .to_string(),
                ))
                .expect("update missing request"),
        )
        .await
        .expect("update missing response");
    assert_eq!(update_missing.status(), StatusCode::NOT_FOUND);
    let update_body = response_json(update_missing).await;
    assert_eq!(update_body["error"]["code"], "NOT_FOUND");

    let delete_missing = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/policy/rules/missing_rule")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete missing request"),
        )
        .await
        .expect("delete missing response");
    assert_eq!(delete_missing.status(), StatusCode::NOT_FOUND);
    let delete_body = response_json(delete_missing).await;
    assert_eq!(delete_body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn policy_rule_request_body_validation_errors_return_invalid_argument() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let unknown_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "unknown_field_rule",
                        "decision": "deny",
                        "reason": "bad payload",
                        "unexpected": "not allowed"
                    })
                    .to_string(),
                ))
                .expect("unknown field request"),
        )
        .await
        .expect("unknown field response");
    assert_eq!(unknown_field.status(), StatusCode::BAD_REQUEST);
    let unknown_field_body = response_json(unknown_field).await;
    assert_eq!(unknown_field_body["error"]["code"], "INVALID_ARGUMENT");

    let malformed_json = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"id":"malformed","decision":"deny","reason":"oops""#,
                ))
                .expect("malformed json request"),
        )
        .await
        .expect("malformed json response");
    assert_eq!(malformed_json.status(), StatusCode::BAD_REQUEST);
    let malformed_body = response_json(malformed_json).await;
    assert_eq!(malformed_body["error"]["code"], "INVALID_ARGUMENT");
}
