use axum::body::Body;
use axum::http::{Request, StatusCode};
use openjax_core::{ApprovalHandler, ApprovalRequest};
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

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

    let resolve_uri = format!(
        "/api/v1/sessions/{}/approvals/{}:resolve",
        session_id, approval_id
    );
    let first_resolve = {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            let response = app
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
            if response.status() == StatusCode::OK {
                break response;
            }
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            if tokio::time::Instant::now() >= deadline {
                panic!("approval did not become pending within timeout");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
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
