use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

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
