use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::helpers::{app_with_api_key, auth_header, login, response_json};

#[tokio::test]
async fn create_policy_rule_rejects_blank_id_or_reason() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let blank_id = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "   ",
                        "decision": "deny",
                        "reason": "block"
                    })
                    .to_string(),
                ))
                .expect("blank id request"),
        )
        .await
        .expect("blank id response");
    assert_eq!(blank_id.status(), StatusCode::BAD_REQUEST);
    let blank_id_body = response_json(blank_id).await;
    assert_eq!(blank_id_body["error"]["code"], "INVALID_ARGUMENT");

    let blank_reason = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "deny_exec_blank_reason",
                        "decision": "deny",
                        "reason": "   "
                    })
                    .to_string(),
                ))
                .expect("blank reason request"),
        )
        .await
        .expect("blank reason response");
    assert_eq!(blank_reason.status(), StatusCode::BAD_REQUEST);
    let blank_reason_body = response_json(blank_reason).await;
    assert_eq!(blank_reason_body["error"]["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn policy_rule_request_body_validation_errors_return_invalid_argument() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let unknown_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "unknown_field_rule",
                        "decision": "deny",
                        "reason": "bad payload",
                        "unexpected": "not allowed"
                    })
                    .to_string(),
                ))
                .expect("unknown field request"),
        )
        .await
        .expect("unknown field response");
    assert_eq!(unknown_field.status(), StatusCode::BAD_REQUEST);
    let unknown_field_body = response_json(unknown_field).await;
    assert_eq!(unknown_field_body["error"]["code"], "INVALID_ARGUMENT");

    let malformed_json = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"id":"malformed","decision":"deny","reason":"oops""#,
                ))
                .expect("malformed json request"),
        )
        .await
        .expect("malformed json response");
    assert_eq!(malformed_json.status(), StatusCode::BAD_REQUEST);
    let malformed_body = response_json(malformed_json).await;
    assert_eq!(malformed_body["error"]["code"], "INVALID_ARGUMENT");
}
