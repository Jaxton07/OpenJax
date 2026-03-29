use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::gateway_api::helpers::{
    app_with_api_key, auth_header, create_session_for_test, login, response_json,
};

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
