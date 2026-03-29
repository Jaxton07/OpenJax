use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tokio::time::sleep;
use tower::ServiceExt;

use super::helpers::{
    app_with_api_key, auth_header, create_session, login, response_json, session_timeline_events,
    submit_turn,
};

#[tokio::test]
async fn policy_rule_create_update_publish_affects_submit_turn() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;
    let session_id = create_session(&app, &access_token).await;

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
                        "id": "read_gate",
                        "decision": "deny",
                        "priority": 120,
                        "tool_name": "Read",
                        "action": "read",
                        "reason": "deny read before review"
                    })
                    .to_string(),
                ))
                .expect("create rule request"),
        )
        .await
        .expect("create rule response");
    assert_eq!(create.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/policy/rules/read_gate")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "decision": "ask",
                        "priority": 140,
                        "tool_name": "Read",
                        "action": "read",
                        "reason": "ask before reading file"
                    })
                    .to_string(),
                ))
                .expect("update rule request"),
        )
        .await
        .expect("update rule response");
    assert_eq!(update.status(), StatusCode::OK);

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/publish")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("publish request"),
        )
        .await
        .expect("publish response");
    assert_eq!(publish.status(), StatusCode::OK);
    let publish_body = response_json(publish).await;
    let published_version = publish_body["policy_version"]
        .as_u64()
        .expect("published policy_version");

    let submit_body = submit_turn(
        &app,
        &access_token,
        &session_id,
        "tool:Read file_path=Cargo.toml",
    )
    .await;
    assert!(submit_body["turn_id"].as_str().is_some());

    let mut approval_event: Option<Value> = None;
    for _ in 0..30 {
        let events = session_timeline_events(&app, &access_token, &session_id).await;
        if let Some(found) = events
            .iter()
            .find(|event| event["type"] == "approval_requested")
        {
            approval_event = Some(found.clone());
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    let approval_event = approval_event.expect("approval_requested event should be persisted");
    assert_eq!(
        approval_event["payload"]["matched_rule_id"],
        Value::String("read_gate".to_string())
    );
    assert_eq!(
        approval_event["payload"]["policy_version"],
        Value::Number(published_version.into())
    );
}
