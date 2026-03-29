use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::helpers::{app_with_api_key, auth_header, login, response_json};

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
