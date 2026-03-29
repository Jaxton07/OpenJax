use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::helpers::{app_with_api_key, auth_header, login, response_json};

#[tokio::test]
async fn policy_rules_crud_roundtrip() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "deny_exec",
                        "decision": "deny",
                        "priority": 100,
                        "tool_name": "exec_command",
                        "action": "exec",
                        "reason": "block command execution by default"
                    })
                    .to_string(),
                ))
                .expect("create rule request"),
        )
        .await
        .expect("create rule response");
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = response_json(create).await;
    assert_eq!(create_body["rule"]["id"], "deny_exec");
    assert_eq!(create_body["rule"]["decision"], "deny");

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list rules request"),
        )
        .await
        .expect("list rules response");
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = response_json(list).await;
    assert_eq!(list_body["rules"].as_array().expect("rules array").len(), 1);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/deny_exec")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "ask",
                        "priority": 80,
                        "tool_name": "exec_command",
                        "action": "exec",
                        "reason": "ask before execution"
                    })
                    .to_string(),
                ))
                .expect("update rule request"),
        )
        .await
        .expect("update rule response");
    assert_eq!(update.status(), StatusCode::OK);
    let update_body = response_json(update).await;
    assert_eq!(update_body["rule"]["decision"], "ask");

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/policy/rules/deny_exec")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete rule request"),
        )
        .await
        .expect("delete rule response");
    assert_eq!(delete.status(), StatusCode::OK);
    let delete_body = response_json(delete).await;
    assert_eq!(delete_body["status"], "deleted");

    let list_after_delete = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list rules request"),
        )
        .await
        .expect("list rules response");
    assert_eq!(list_after_delete.status(), StatusCode::OK);
    let list_after_delete_body = response_json(list_after_delete).await;
    assert_eq!(
        list_after_delete_body["rules"]
            .as_array()
            .expect("rules array")
            .len(),
        0
    );
}

#[tokio::test]
async fn create_policy_rule_rejects_duplicate_rule_id() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let create_once = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "duplicate_rule",
                        "decision": "ask",
                        "reason": "first create"
                    })
                    .to_string(),
                ))
                .expect("first create request"),
        )
        .await
        .expect("first create response");
    assert_eq!(create_once.status(), StatusCode::OK);

    let create_twice = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/rules")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "id": "duplicate_rule",
                        "decision": "deny",
                        "reason": "second create"
                    })
                    .to_string(),
                ))
                .expect("second create request"),
        )
        .await
        .expect("second create response");
    assert_eq!(create_twice.status(), StatusCode::CONFLICT);
    let body = response_json(create_twice).await;
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn update_and_delete_nonexistent_policy_rule_return_not_found() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;

    let update_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/missing_rule")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "deny",
                        "reason": "no such rule"
                    })
                    .to_string(),
                ))
                .expect("update missing request"),
        )
        .await
        .expect("update missing response");
    assert_eq!(update_missing.status(), StatusCode::NOT_FOUND);
    let update_body = response_json(update_missing).await;
    assert_eq!(update_body["error"]["code"], "NOT_FOUND");

    let delete_missing = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/policy/rules/missing_rule")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete missing request"),
        )
        .await
        .expect("delete missing response");
    assert_eq!(delete_missing.status(), StatusCode::NOT_FOUND);
    let delete_body = response_json(delete_missing).await;
    assert_eq!(delete_body["error"]["code"], "NOT_FOUND");
}
