use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde::Serialize;

use crate::error::ApiError;
use crate::middleware::RequestContext;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProviderItem {
    provider_id: String,
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key_set: bool,
    provider_type: String,
    context_window_size: u32,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderListResponse {
    request_id: String,
    providers: Vec<ProviderItem>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderMutationResponse {
    request_id: String,
    provider: ProviderItem,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderDeleteResponse {
    request_id: String,
    provider_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveProviderItem {
    provider_id: String,
    model_name: String,
    context_window_size: u32,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveProviderResponse {
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_provider: Option<ActiveProviderItem>,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key: String,
    #[serde(default = "default_provider_type")]
    provider_type: String,
    #[serde(default)]
    context_window_size: u32,
}

fn default_provider_type() -> String {
    "custom".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    provider_name: String,
    base_url: String,
    model_name: String,
    api_key: Option<String>,
    #[serde(default)]
    context_window_size: u32,
}

#[derive(Debug, Deserialize)]
pub struct SetActiveProviderRequest {
    provider_id: String,
}

// ---- Catalog ----

#[derive(Debug, Serialize)]
pub struct CatalogModelItem {
    model_id: &'static str,
    display_name: &'static str,
    context_window: u32,
}

#[derive(Debug, Serialize)]
pub struct CatalogProviderItem {
    catalog_key: &'static str,
    display_name: &'static str,
    base_url: &'static str,
    protocol: &'static str,
    default_model: &'static str,
    models: Vec<CatalogModelItem>,
}

#[derive(Debug, Serialize)]
pub struct CatalogResponse {
    providers: Vec<CatalogProviderItem>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_provider_item(provider: openjax_store::ProviderRecord) -> ProviderItem {
    ProviderItem {
        provider_id: provider.provider_id,
        provider_name: provider.provider_name,
        base_url: provider.base_url,
        model_name: provider.model_name,
        api_key_set: !provider.api_key.trim().is_empty(),
        provider_type: provider.provider_type,
        context_window_size: provider.context_window_size,
        created_at: provider.created_at,
        updated_at: provider.updated_at,
    }
}

fn to_active_provider_item(active: openjax_store::ActiveProviderRecord) -> ActiveProviderItem {
    ActiveProviderItem {
        provider_id: active.provider_id,
        model_name: active.model_name,
        context_window_size: active.context_window_size,
        updated_at: active.updated_at,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn list_providers(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ProviderListResponse>, ApiError> {
    let providers = state
        .list_providers()?
        .into_iter()
        .map(to_provider_item)
        .collect::<Vec<ProviderItem>>();
    Ok(Json(ProviderListResponse {
        request_id: ctx.request_id,
        providers,
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn get_active_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ActiveProviderResponse>, ApiError> {
    let active_provider = state.get_active_provider()?.map(to_active_provider_item);
    Ok(Json(ActiveProviderResponse {
        request_id: ctx.request_id,
        active_provider,
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn set_active_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<SetActiveProviderRequest>,
) -> Result<Json<ActiveProviderResponse>, ApiError> {
    let provider_id = payload.provider_id.trim();
    if provider_id.is_empty() {
        return Err(ApiError::invalid_argument(
            "provider_id must not be empty",
            serde_json::json!({}),
        ));
    }
    let selected = state.set_active_provider(provider_id)?.ok_or_else(|| {
        ApiError::not_found(
            "provider not found",
            serde_json::json!({ "provider_id": provider_id }),
        )
    })?;
    Ok(Json(ActiveProviderResponse {
        request_id: ctx.request_id,
        active_provider: Some(to_active_provider_item(selected)),
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn create_provider(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateProviderRequest>,
) -> Result<Json<ProviderMutationResponse>, ApiError> {
    let provider_name = payload.provider_name.trim();
    let base_url = payload.base_url.trim();
    let model_name = payload.model_name.trim();
    let api_key = payload.api_key.trim();
    if provider_name.is_empty()
        || base_url.is_empty()
        || model_name.is_empty()
        || api_key.is_empty()
    {
        return Err(ApiError::invalid_argument(
            "provider fields must not be empty",
            serde_json::json!({}),
        ));
    }
    let created = state.create_provider(
        provider_name,
        base_url,
        model_name,
        api_key,
        &payload.provider_type,
        payload.context_window_size,
    )?;
    Ok(Json(ProviderMutationResponse {
        request_id: ctx.request_id,
        provider: to_provider_item(created),
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn update_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderMutationResponse>, ApiError> {
    let provider_name = payload.provider_name.trim();
    let base_url = payload.base_url.trim();
    let model_name = payload.model_name.trim();
    let api_key = payload
        .api_key
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if provider_name.is_empty() || base_url.is_empty() || model_name.is_empty() {
        return Err(ApiError::invalid_argument(
            "provider fields must not be empty",
            serde_json::json!({}),
        ));
    }
    let updated = state
        .update_provider(
            &provider_id,
            provider_name,
            base_url,
            model_name,
            api_key,
            payload.context_window_size,
        )?
        .ok_or_else(|| {
            ApiError::not_found(
                "provider not found",
                serde_json::json!({ "provider_id": provider_id }),
            )
        })?;
    Ok(Json(ProviderMutationResponse {
        request_id: ctx.request_id,
        provider: to_provider_item(updated),
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ProviderDeleteResponse>, ApiError> {
    let deleted = state.delete_provider(&provider_id)?;
    if !deleted {
        return Err(ApiError::not_found(
            "provider not found",
            serde_json::json!({ "provider_id": provider_id }),
        ));
    }
    Ok(Json(ProviderDeleteResponse {
        request_id: ctx.request_id,
        provider_id,
        status: "deleted",
        timestamp: crate::error::now_rfc3339(),
    }))
}

pub async fn get_catalog() -> impl IntoResponse {
    use openjax_core::BUILTIN_CATALOG;
    let providers = BUILTIN_CATALOG
        .iter()
        .map(|p| CatalogProviderItem {
            catalog_key: p.catalog_key,
            display_name: p.display_name,
            base_url: p.base_url,
            protocol: p.protocol,
            default_model: p.default_model,
            models: p
                .models
                .iter()
                .map(|m| CatalogModelItem {
                    model_id: m.model_id,
                    display_name: m.display_name,
                    context_window: m.context_window,
                })
                .collect(),
        })
        .collect();
    Json(CatalogResponse { providers })
}
