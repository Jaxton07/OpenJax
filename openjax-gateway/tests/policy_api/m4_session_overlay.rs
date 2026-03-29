use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::helpers::{app_with_api_key, auth_header, create_session, login, response_json};

#[tokio::test]
async fn session_overlay_set_and_clear() {
    let api_key = "test-key";
    let app = app_with_api_key(api_key);
    let access_token = login(&app, api_key).await;
    let session_id = create_session(&app, &access_token).await;

    let set_overlay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/sessions/{}/policy-overlay", session_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "rules": [
                            {
                                "id": "overlay_deny_exec",
                                "decision": "deny",
                                "priority": 120,
                                "tool_name": "exec_command",
                                "action": "exec",
                                "reason": "deny this session"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("set overlay request"),
        )
        .await
        .expect("set overlay response");
    assert_eq!(set_overlay.status(), StatusCode::OK);
    let set_body = response_json(set_overlay).await;
    assert_eq!(set_body["status"], "set");
    assert_eq!(set_body["rule_count"], 1);
    let set_version = set_body["policy_version"]
        .as_u64()
        .expect("set policy version");

    let clear_overlay = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{}/policy-overlay", session_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("clear overlay request"),
        )
        .await
        .expect("clear overlay response");
    assert_eq!(clear_overlay.status(), StatusCode::OK);
    let clear_body = response_json(clear_overlay).await;
    assert_eq!(clear_body["status"], "cleared");
    assert_eq!(clear_body["rule_count"], 0);
    let clear_version = clear_body["policy_version"]
        .as_u64()
        .expect("clear policy version");
    assert_eq!(clear_version, set_version + 1);
}
