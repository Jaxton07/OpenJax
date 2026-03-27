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
    let state = AppState::new_with_api_keys_for_test(keys);
    let app = build_app(state.clone(), None);
    (app, state)
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

async fn login(app: &axum::Router, owner_key: &str) -> (String, String, String) {
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
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .expect("set-cookie header")
        .to_string();
    let body = response_json(response).await;
    let access = body["access_token"]
        .as_str()
        .expect("access token")
        .to_string();
    let session_id = body["session_id"].as_str().expect("session id").to_string();
    (access, set_cookie, session_id)
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
async fn login_refresh_logout_flow() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);

    let (access_token, set_cookie, session_id) = login(&app, api_key).await;

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
    assert_eq!(create_response.status(), StatusCode::OK);

    let refresh_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", set_cookie)
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("refresh request"),
        )
        .await
        .expect("refresh response");
    assert_eq!(refresh_response.status(), StatusCode::OK);

    let logout_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "session_id": session_id }).to_string(),
                ))
                .expect("logout request"),
        )
        .await
        .expect("logout response");
    assert_eq!(logout_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn logout_without_access_token_returns_401() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (_access_token, _cookie, session_id) = login(&app, api_key).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "session_id": session_id }).to_string(),
                ))
                .expect("logout request"),
        )
        .await
        .expect("logout response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "UNAUTHENTICATED");
}

#[tokio::test]
async fn refresh_reuse_conflict_returns_conflict() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (_access_token, old_cookie, _session_id) = login(&app, api_key).await;

    let first_refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", old_cookie.clone())
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("refresh request"),
        )
        .await
        .expect("first refresh response");
    assert_eq!(first_refresh.status(), StatusCode::OK);

    let second_refresh = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("Cookie", old_cookie)
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("second refresh request"),
        )
        .await
        .expect("second refresh response");
    assert_eq!(second_refresh.status(), StatusCode::CONFLICT);
    let body = response_json(second_refresh).await;
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn revoke_session_invalidates_access_token() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _cookie, session_id) = login(&app, api_key).await;

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/revoke")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "session_id": session_id }).to_string(),
                ))
                .expect("revoke request"),
        )
        .await
        .expect("revoke response");
    assert_eq!(revoke_response.status(), StatusCode::OK);

    let create_response = app
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
    assert_eq!(create_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn clear_command_submit_and_polling_flow() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
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
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"].as_str().expect("session_id");

    // Use the new /slash endpoint instead of submit_turn with /clear
    let slash_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}/slash", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"command":"clear"}"#))
                .expect("slash request"),
        )
        .await
        .expect("slash response");
    assert_eq!(slash_response.status(), StatusCode::OK);
    let slash_body = response_json(slash_response).await;
    assert_eq!(slash_body["status"], "ok");
    assert_eq!(slash_body["message"], "session cleared");
    // No turn_id or polling needed - /slash does not create turn events
}

#[tokio::test]
async fn slash_commands_endpoint_returns_aliases_and_replaces_input() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/slash_commands")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("slash commands request"),
        )
        .await
        .expect("slash commands response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;

    let commands = body["commands"].as_array().expect("commands array");
    let help = commands
        .iter()
        .find(|item| item["name"] == "help")
        .expect("help command present");
    assert_eq!(help["usage_hint"], "/help");
    assert_eq!(help["replaces_input"], false);
    assert_eq!(
        help["aliases"].as_array().expect("help aliases"),
        &vec![Value::String("?".to_string())]
    );

    let clear = commands
        .iter()
        .find(|item| item["name"] == "clear")
        .expect("clear command present");
    assert_eq!(clear["usage_hint"], "/clear");
    assert_eq!(clear["replaces_input"], false);
    assert_eq!(
        clear["aliases"].as_array().expect("clear aliases"),
        &vec![Value::String("cls".to_string())]
    );
}

#[tokio::test]
async fn compact_endpoint_succeeds() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
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
    let session_id = create_body["session_id"].as_str().expect("session_id");

    let compact_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{}:compact", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"strategy":"default"}"#))
                .expect("compact request"),
        )
        .await
        .expect("compact response");
    assert_eq!(compact_response.status(), StatusCode::OK);
    let compact_body = response_json(compact_response).await;
    assert_eq!(compact_body["status"], "compacted");
}

#[tokio::test]
async fn approval_resolve_second_call_returns_conflict() {
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
                .header("Authorization", auth_header(&access_token))
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
                .header("Authorization", auth_header(&access_token))
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
    assert!(waiter_result.expect("approval resolved"));
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
                "response_text_delta",
                serde_json::json!({ "idx": i }),
                None,
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
                "response_text_delta",
                serde_json::json!({ "idx": i }),
                None,
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
async fn shutdown_session_endpoint_returns_shutdown_status() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
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
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"].as_str().expect("session_id");

    let shutdown_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{}", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("shutdown request"),
        )
        .await
        .expect("shutdown response");
    assert_eq!(shutdown_response.status(), StatusCode::OK);
    let shutdown_body = response_json(shutdown_response).await;
    assert_eq!(shutdown_body["status"], "shutdown");
}

#[tokio::test]
async fn provider_crud_endpoints_work() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/providers")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider_name": "openai-main",
                        "base_url": "https://api.openai.com/v1",
                        "model_name": "gpt-4.1-mini",
                        "api_key": "sk-test"
                    })
                    .to_string(),
                ))
                .expect("create provider request"),
        )
        .await
        .expect("create provider response");
    assert_eq!(create_response.status(), StatusCode::OK);
    let created = response_json(create_response).await;
    let provider_id = created["provider"]["provider_id"]
        .as_str()
        .expect("provider_id")
        .to_string();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/providers")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list providers request"),
        )
        .await
        .expect("list providers response");
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["providers"][0]["provider_name"], "openai-main");
    assert_eq!(listed["providers"][0]["api_key_set"], true);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/providers/{}", provider_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider_name": "openai-main",
                        "base_url": "https://api.openai.com/v1",
                        "model_name": "gpt-4.1",
                        "api_key": ""
                    })
                    .to_string(),
                ))
                .expect("update provider request"),
        )
        .await
        .expect("update provider response");
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["provider"]["model_name"], "gpt-4.1");

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/providers/{}", provider_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete provider request"),
        )
        .await
        .expect("delete provider response");
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted = response_json(delete_response).await;
    assert_eq!(deleted["status"], "deleted");
}

#[tokio::test]
async fn get_session_rehydrates_from_persistence() {
    let (app, state) = app_with_api_key("test-key");
    let (access_token, _, _) = login(&app, "test-key").await;
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
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = response_json(create_response).await;
    let session_id = body["session_id"].as_str().expect("session id").to_string();
    state.remove_session(&session_id).await;
    let recovered = state.get_session(&session_id).await;
    assert!(recovered.is_ok());
}

#[tokio::test]
async fn session_messages_endpoint_returns_persisted_messages() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
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
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"]
        .as_str()
        .expect("session_id")
        .to_string();

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
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"]
        .as_str()
        .expect("session_id")
        .to_string();

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

// ---------------------------------------------------------------------------
// Policy level tests
// ---------------------------------------------------------------------------

async fn create_session_for_test(app: &axum::Router, access_token: &str) -> String {
    let resp = app
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
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp).await;
    body["session_id"].as_str().expect("session_id").to_string()
}

#[tokio::test]
async fn put_policy_valid_level_returns_200() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{}/policy", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"level":"allow"}"#))
                .expect("put policy request"),
        )
        .await
        .expect("put policy response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp).await;
    assert_eq!(body["level"], "allow");
}

#[tokio::test]
async fn put_policy_invalid_level_returns_400() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{}/policy", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"level":"ultra"}"#))
                .expect("put policy request"),
        )
        .await
        .expect("put policy response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_policy_level_returns_200_with_default_level() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/policy", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("get policy request"),
        )
        .await
        .expect("get policy response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp).await;
    assert_eq!(body["session_id"], session_id);
    let level = body["level"].as_str().expect("level field");
    assert!(
        ["allow", "ask", "deny"].contains(&level),
        "unexpected level: {level}"
    );
}

#[tokio::test]
async fn get_policy_level_reflects_put_change() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let session_id = create_session_for_test(&app, &access_token).await;

    let put_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{}/policy", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"level":"allow"}"#))
                .expect("put policy request"),
        )
        .await
        .expect("put policy response");
    assert_eq!(put_resp.status(), StatusCode::OK);

    let get_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{}/policy", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("get policy request"),
        )
        .await
        .expect("get policy response");
    assert_eq!(get_resp.status(), StatusCode::OK);
    let body = response_json(get_resp).await;
    assert_eq!(body["level"], "allow");
}
