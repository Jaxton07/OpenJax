use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

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
    assert_eq!(messages_body["messages"][0]["role"], "user");
}
