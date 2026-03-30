use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

#[tokio::test]
async fn shutdown_session_endpoint_returns_shutdown_status() {
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
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let session_id = create_body["session_id"].as_str().expect("session_id").to_string();

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

    let transcript_session_root = state
        .transcript
        .root()
        .join("sessions")
        .join(&session_id);
    assert!(
        transcript_session_root.exists(),
        "transcript session dir should exist before shutdown"
    );

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
    assert!(
        !transcript_session_root.exists(),
        "transcript session dir should be deleted after shutdown"
    );
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

#[tokio::test]
async fn list_sessions_supports_cursor_limit_and_next_cursor() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    for _ in 0..3 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("Authorization", auth_header(&access_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .expect("create session request"),
            )
            .await
            .expect("create session response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let first_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sessions?limit=2")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list sessions page1"),
        )
        .await
        .expect("list page1 response");
    assert_eq!(first_page.status(), StatusCode::OK);
    let first_body = response_json(first_page).await;
    let first_sessions = first_body["sessions"].as_array().expect("sessions array");
    assert_eq!(first_sessions.len(), 2);
    let next_cursor = first_body["next_cursor"]
        .as_str()
        .expect("next_cursor should exist on first page");

    let second_page = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions?limit=2&cursor={next_cursor}"))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list sessions page2"),
        )
        .await
        .expect("list page2 response");
    assert_eq!(second_page.status(), StatusCode::OK);
    let second_body = response_json(second_page).await;
    let second_sessions = second_body["sessions"].as_array().expect("sessions array");
    assert_eq!(second_sessions.len(), 1);
}

#[tokio::test]
async fn list_sessions_rejects_invalid_cursor() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sessions?cursor=***invalid***")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list sessions request"),
        )
        .await
        .expect("list sessions response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn sessions_endpoints_return_503_when_index_repair_required() {
    let api_key = "test-key";
    let (app, state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;
    let create_for_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("create before repair-required"),
        )
        .await
        .expect("create before repair-required response");
    assert_eq!(create_for_delete.status(), StatusCode::OK);
    let create_for_delete_body = response_json(create_for_delete).await;
    let session_id = create_for_delete_body["session_id"]
        .as_str()
        .expect("session_id")
        .to_string();

    state
        .session_index
        .force_repair_required_for_test()
        .expect("force repair required");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sessions")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list request"),
        )
        .await
        .expect("list response");
    assert_eq!(list_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body["error"]["code"], "INDEX_REPAIR_REQUIRED");

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
    assert_eq!(create_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let create_body = response_json(create_response).await;
    assert_eq!(create_body["error"]["code"], "INDEX_REPAIR_REQUIRED");

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete response");
    assert_eq!(delete_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let delete_body = response_json(delete_response).await;
    assert_eq!(delete_body["error"]["code"], "INDEX_REPAIR_REQUIRED");
}
