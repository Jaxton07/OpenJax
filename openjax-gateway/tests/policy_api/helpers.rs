use std::collections::HashSet;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use openjax_gateway::{AppState, build_app};
use serde_json::Value;
use tower::ServiceExt;

pub(crate) fn app_with_api_key(api_key: &str) -> axum::Router {
    let mut keys = HashSet::new();
    keys.insert(api_key.to_string());
    let state = AppState::new_with_api_keys_for_test(keys);
    build_app(state, None)
}

pub(crate) fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}

pub(crate) async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

pub(crate) async fn login(app: &axum::Router, owner_key: &str) -> String {
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

pub(crate) async fn create_session(app: &axum::Router, access_token: &str) -> String {
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

pub(crate) async fn submit_turn(
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

pub(crate) async fn session_timeline_events(
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
