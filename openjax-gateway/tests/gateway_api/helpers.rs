use std::collections::HashSet;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use openjax_gateway::{AppState, build_app};
use serde_json::Value;
use tower::ServiceExt;

#[allow(dead_code)]
pub(crate) fn app_with_api_key(api_key: &str) -> (axum::Router, AppState) {
    let mut keys = HashSet::new();
    keys.insert(api_key.to_string());
    let state = AppState::new_with_api_keys_for_test(keys);
    let app = build_app(state.clone(), None);
    (app, state)
}

pub(crate) fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}

#[allow(dead_code)]
pub(crate) async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response json")
}

#[allow(dead_code)]
pub(crate) async fn login(app: &axum::Router, owner_key: &str) -> (String, String, String) {
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

#[allow(dead_code)]
pub(crate) async fn create_session_for_test(app: &axum::Router, access_token: &str) -> String {
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
