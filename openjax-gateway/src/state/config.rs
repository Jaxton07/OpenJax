//! Config helpers and environment-based settings.

use std::path::PathBuf;

use openjax_core::OpenJaxPaths;
use openjax_store::SqliteStore;
use serde_json::json;

use crate::error::ApiError;

pub const DEFAULT_EVENT_REPLAY_LIMIT: usize = 1024;
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 1024;

pub fn event_replay_limit() -> usize {
    std::env::var("OPENJAX_GATEWAY_EVENT_REPLAY_LIMIT")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EVENT_REPLAY_LIMIT)
}

pub fn event_channel_capacity() -> usize {
    std::env::var("OPENJAX_GATEWAY_EVENT_CHANNEL_CAPACITY")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EVENT_CHANNEL_CAPACITY)
}

pub fn gateway_db_path() -> PathBuf {
    OpenJaxPaths::detect()
        .map(|paths| {
            let _ = paths.ensure_runtime_dirs();
            paths.database_dir.join("gateway.db")
        })
        .unwrap_or_else(|| PathBuf::from(".openjax/database/gateway.db"))
}

pub fn map_store_error(err: anyhow::Error) -> ApiError {
    let text = err.to_string();
    if text.contains("UNIQUE constraint failed") {
        return ApiError::conflict("duplicate resource", json!({ "reason": text }));
    }
    ApiError::internal(text)
}

pub fn build_runtime_config(
    providers: Vec<openjax_store::ProviderRecord>,
    active_provider_id: Option<&str>,
) -> openjax_core::Config {
    openjax_core::build_config_from_providers(providers, active_provider_id)
}

pub fn migrate_providers_from_config_if_needed(store: &SqliteStore) {
    use openjax_store::ProviderRepository;
    let existing = store.list_providers().unwrap_or_default();
    if !existing.is_empty() {
        return;
    }
    let config = openjax_core::Config::load();
    let Some(model) = config.model else {
        return;
    };
    for (model_id, entry) in model.models {
        let api_key = entry
            .api_key
            .or_else(|| {
                entry
                    .api_key_env
                    .as_ref()
                    .and_then(|env_name| std::env::var(env_name).ok())
            })
            .unwrap_or_default();
        if api_key.trim().is_empty() {
            continue;
        }
        let base_url = entry
            .base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let model_name = entry.model.unwrap_or_else(|| model_id.clone());
        let _ = store.create_provider(&model_id, &base_url, &model_name, &api_key, "custom", 0);
    }
}
