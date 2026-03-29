use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::gateway_api::helpers::{app_with_api_key, auth_header, login, response_json};

#[tokio::test]
async fn provider_crud_endpoints_work() {
    let api_key = "test-key";
    let (app, _state) = app_with_api_key(api_key);
    let (access_token, _, _) = login(&app, api_key).await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/providers")
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider_name": "openai-main",
                        "base_url": "https://api.openai.com/v1",
                        "model_name": "gpt-4.1-mini",
                        "api_key": "sk-test"
                    })
                    .to_string(),
                ))
                .expect("create provider request"),
        )
        .await
        .expect("create provider response");
    assert_eq!(create_response.status(), StatusCode::OK);
    let created = response_json(create_response).await;
    let provider_id = created["provider"]["provider_id"]
        .as_str()
        .expect("provider_id")
        .to_string();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/providers")
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("list providers request"),
        )
        .await
        .expect("list providers response");
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    let providers = listed["providers"].as_array().expect("providers array");
    let listed_provider = providers
        .iter()
        .find(|provider| {
            provider["provider_id"].as_str() == Some(provider_id.as_str())
                || provider["provider_name"].as_str() == Some("openai-main")
        })
        .expect("created provider in list");
    assert_eq!(listed_provider["provider_name"], "openai-main");
    assert_eq!(listed_provider["api_key_set"], true);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/providers/{}", provider_id))
                .header("Authorization", auth_header(&access_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider_name": "openai-main",
                        "base_url": "https://api.openai.com/v1",
                        "model_name": "gpt-4.1",
                        "api_key": ""
                    })
                    .to_string(),
                ))
                .expect("update provider request"),
        )
        .await
        .expect("update provider response");
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["provider"]["model_name"], "gpt-4.1");

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/providers/{}", provider_id))
                .header("Authorization", auth_header(&access_token))
                .body(Body::empty())
                .expect("delete provider request"),
        )
        .await
        .expect("delete provider response");
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted = response_json(delete_response).await;
    assert_eq!(deleted["status"], "deleted");
}
