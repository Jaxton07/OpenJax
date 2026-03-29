use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

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
