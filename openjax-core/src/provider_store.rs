use std::collections::HashMap;

use openjax_store::{ProviderRecord, ProviderRepository, SqliteStore};

use crate::config::{Config, ModelConfig, ModelRoutingConfig, ProviderModelConfig};
use crate::paths::OpenJaxPaths;

/// Load LLM provider config from the shared gateway DB and merge it with the
/// non-model portions of the file-based config (sandbox / agent / skills).
///
/// This is the single source-of-truth config loader for any process that needs
/// to talk to a model — both the TUI and the gateway daemon use this path so
/// that WebUI-configured providers are always honoured.
pub fn load_runtime_config() -> Config {
    let (providers, active_id) = read_db_providers();
    build_config_from_providers(providers, active_id.as_deref())
}

// ---------------------------------------------------------------------------
// DB reading
// ---------------------------------------------------------------------------

fn gateway_db_path() -> Option<std::path::PathBuf> {
    let paths = OpenJaxPaths::detect()?;
    let db = paths.database_dir.join("gateway.db");
    db.exists().then_some(db)
}

fn read_db_providers() -> (Vec<ProviderRecord>, Option<String>) {
    let db_path = match gateway_db_path() {
        Some(p) => p,
        None => return (Vec::new(), None),
    };
    let store = match SqliteStore::open(&db_path) {
        Ok(s) => s,
        Err(_) => return (Vec::new(), None),
    };
    let providers = store.list_providers().unwrap_or_default();
    let active_id = store
        .get_active_provider()
        .ok()
        .flatten()
        .map(|r| r.provider_id);
    (providers, active_id)
}

// ---------------------------------------------------------------------------
// Config building (canonical implementation shared with gateway/state.rs)
// ---------------------------------------------------------------------------

pub fn build_config_from_providers(
    providers: Vec<ProviderRecord>,
    active_provider_id: Option<&str>,
) -> Config {
    // sandbox / agent / skills still come from the config file; model section is DB-only.
    let mut config = Config::load();
    config.model = None;

    if providers.is_empty() {
        return config;
    }

    // Put active provider first so it becomes the primary planner route.
    let mut ordered = providers;
    if let Some(active_id) = active_provider_id
        && let Some(index) = ordered.iter().position(|p| p.provider_id == active_id)
    {
        let selected = ordered.remove(index);
        ordered.insert(0, selected);
    }

    let mut models: HashMap<String, ProviderModelConfig> = HashMap::new();
    let mut route_order: Vec<String> = Vec::new();

    for provider in ordered {
        let request_profile = infer_request_profile(&provider);
        let mut model_id = normalize_model_id(&provider.provider_name);
        if model_id.is_empty() {
            model_id = format!("provider_{}", provider.provider_id);
        }
        let mut dedup = 1usize;
        while models.contains_key(&model_id) {
            dedup += 1;
            model_id = format!("{}_{}", normalize_model_id(&provider.provider_name), dedup);
        }
        route_order.push(model_id.clone());
        models.insert(
            model_id,
            ProviderModelConfig {
                provider: Some(
                    provider_vendor(&provider.base_url, &provider.provider_name).to_string(),
                ),
                protocol: Some(provider.protocol.clone()),
                model: Some(provider.model_name),
                base_url: Some(provider.base_url),
                api_key: Some(provider.api_key),
                api_key_env: None,
                request_profile,
                anthropic_version: None,
                thinking_budget_tokens: Some(2000),
                supports_stream: Some(true),
                supports_reasoning: Some(true),
                supports_tool_call: Some(true),
                supports_json_mode: Some(false),
                context_window_size: Some(provider.context_window_size),
            },
        );
    }

    let planner = route_order[0].clone();
    let mut fallbacks: HashMap<String, Vec<String>> = HashMap::new();
    for (i, model_id) in route_order.iter().enumerate() {
        let tail: Vec<String> = route_order.iter().skip(i + 1).cloned().collect();
        if !tail.is_empty() {
            fallbacks.insert(model_id.clone(), tail);
        }
    }

    config.model = Some(ModelConfig {
        backend: None,
        api_key: None,
        base_url: None,
        model: None,
        models,
        routing: Some(ModelRoutingConfig {
            planner: Some(planner.clone()),
            final_writer: Some(planner.clone()),
            tool_reasoning: Some(planner),
            fallbacks,
        }),
    });
    config
}

// ---------------------------------------------------------------------------
// Shared helpers — also used by gateway/state.rs to avoid duplication
// ---------------------------------------------------------------------------

pub fn normalize_model_id(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    s.trim_matches('_').to_string()
}

pub fn provider_protocol(base_url: &str, provider_name: &str) -> &'static str {
    let marker = format!("{base_url} {provider_name}").to_ascii_lowercase();
    if marker.contains("anthropic_messages")
        || marker.contains("protocol=anthropic")
        || marker.contains("/v1/messages")
    {
        "anthropic_messages"
    } else {
        "chat_completions"
    }
}

pub fn provider_vendor(base_url: &str, provider_name: &str) -> &'static str {
    let marker = format!("{base_url} {provider_name}").to_ascii_lowercase();
    if marker.contains("anthropic") || marker.contains("claude") {
        "anthropic"
    } else if marker.contains("kimi") {
        "kimi"
    } else if marker.contains("glm") || marker.contains("bigmodel") {
        "glm"
    } else {
        "openai"
    }
}

fn infer_request_profile(provider: &ProviderRecord) -> Option<String> {
    if provider.protocol == "anthropic_messages" {
        return Some("anthropic_default".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use openjax_store::ProviderRecord;

    use super::build_config_from_providers;

    fn sample_provider(
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        protocol: &str,
    ) -> ProviderRecord {
        ProviderRecord {
            provider_id: "provider_1".to_string(),
            provider_name: provider_name.to_string(),
            base_url: base_url.to_string(),
            model_name: model_name.to_string(),
            api_key: "secret".to_string(),
            provider_type: "built_in".to_string(),
            protocol: protocol.to_string(),
            context_window_size: 256_000,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn kimi_provider_infers_anthropic_profile_when_using_anthropic_messages_protocol() {
        let config = build_config_from_providers(
            vec![sample_provider(
                "Kimi Coding",
                "https://api.kimi.com/coding/v1",
                "kimi-for-coding",
                "anthropic_messages",
            )],
            Some("provider_1"),
        );

        let profile = config
            .model
            .as_ref()
            .and_then(|model| model.models.get("kimi_coding"))
            .and_then(|entry| entry.request_profile.as_deref());
        assert_eq!(profile, Some("anthropic_default"));
    }

    #[test]
    fn non_kimi_provider_leaves_request_profile_empty() {
        let config = build_config_from_providers(
            vec![sample_provider(
                "OpenAI",
                "https://api.openai.com/v1",
                "gpt-4.1-mini",
                "chat_completions",
            )],
            Some("provider_1"),
        );

        let profile = config
            .model
            .as_ref()
            .and_then(|model| model.models.get("openai"))
            .and_then(|entry| entry.request_profile.as_deref());
        assert_eq!(profile, None);
    }

    #[test]
    fn anthropic_provider_infers_anthropic_profile_without_db_field() {
        let config = build_config_from_providers(
            vec![sample_provider(
                "Anthropic",
                "https://api.anthropic.com",
                "claude-sonnet-4-6",
                "anthropic_messages",
            )],
            Some("provider_1"),
        );

        let profile = config
            .model
            .as_ref()
            .and_then(|model| model.models.get("anthropic"))
            .and_then(|entry| entry.request_profile.as_deref());
        assert_eq!(profile, Some("anthropic_default"));
    }
}
