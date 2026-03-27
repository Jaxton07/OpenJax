use std::collections::HashMap;

use crate::config::{ModelConfig, ProviderModelConfig};
use crate::model::types::CapabilityFlags;

const DEFAULT_MODEL_ID: &str = "default";
const MAX_FALLBACK_CHAIN: usize = 2;

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    pub models: HashMap<String, RegisteredModel>,
    pub routing: RoutingPlan,
    pub used_legacy_bridge: bool,
    pub has_legacy_fields: bool,
}

#[derive(Debug, Clone)]
pub struct RegisteredModel {
    pub id: String,
    pub provider: String,
    pub protocol: String,
    pub model: String,
    pub request_profile: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub anthropic_version: Option<String>,
    pub thinking_budget_tokens: Option<u32>,
    pub capabilities: CapabilityFlags,
}

#[derive(Debug, Clone)]
pub struct RoutingPlan {
    pub planner: String,
    pub final_writer: String,
    pub tool_reasoning: String,
    #[allow(dead_code)]
    pub fallbacks: HashMap<String, Vec<String>>,
    #[allow(dead_code)]
    pub max_fallback_chain: usize,
}

impl ModelRegistry {
    pub fn from_config(config: Option<&ModelConfig>) -> Self {
        let Some(config) = config else {
            return Self::empty();
        };

        let has_legacy_fields = config.backend.is_some()
            || config.api_key.is_some()
            || config.base_url.is_some()
            || config.model.is_some();

        if !config.models.is_empty() {
            let models = config
                .models
                .iter()
                .map(|(id, entry)| {
                    (
                        id.to_string(),
                        normalize_model_entry(id, entry, config.api_key.as_ref()),
                    )
                })
                .collect::<HashMap<_, _>>();

            let default_model_id = models
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| DEFAULT_MODEL_ID.to_string());

            let routing_cfg = config.routing.clone().unwrap_or_default();
            let planner = routing_cfg
                .planner
                .unwrap_or_else(|| default_model_id.clone());
            let final_writer = routing_cfg.final_writer.unwrap_or_else(|| planner.clone());
            let tool_reasoning = routing_cfg
                .tool_reasoning
                .unwrap_or_else(|| planner.clone());

            return Self {
                models,
                routing: RoutingPlan {
                    planner,
                    final_writer,
                    tool_reasoning,
                    fallbacks: routing_cfg.fallbacks,
                    max_fallback_chain: MAX_FALLBACK_CHAIN,
                },
                used_legacy_bridge: has_legacy_fields,
                has_legacy_fields,
            };
        }

        if has_legacy_fields {
            let backend = config
                .backend
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_else(|| "openai".to_string());

            let protocol = infer_protocol(&backend);
            let provider = backend.clone();
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| default_model_name(&backend).to_string());
            let entry = RegisteredModel {
                id: DEFAULT_MODEL_ID.to_string(),
                provider: provider.clone(),
                protocol: protocol.to_string(),
                model,
                request_profile: None,
                base_url: config.base_url.clone(),
                api_key: config.api_key.clone(),
                anthropic_version: None,
                thinking_budget_tokens: None,
                capabilities: default_capabilities(protocol),
            };

            let mut models = HashMap::new();
            models.insert(DEFAULT_MODEL_ID.to_string(), entry);

            return Self {
                models,
                routing: RoutingPlan {
                    planner: DEFAULT_MODEL_ID.to_string(),
                    final_writer: DEFAULT_MODEL_ID.to_string(),
                    tool_reasoning: DEFAULT_MODEL_ID.to_string(),
                    fallbacks: HashMap::new(),
                    max_fallback_chain: MAX_FALLBACK_CHAIN,
                },
                used_legacy_bridge: true,
                has_legacy_fields: true,
            };
        }

        Self::empty()
    }

    fn empty() -> Self {
        Self {
            models: HashMap::new(),
            routing: RoutingPlan {
                planner: DEFAULT_MODEL_ID.to_string(),
                final_writer: DEFAULT_MODEL_ID.to_string(),
                tool_reasoning: DEFAULT_MODEL_ID.to_string(),
                fallbacks: HashMap::new(),
                max_fallback_chain: MAX_FALLBACK_CHAIN,
            },
            used_legacy_bridge: false,
            has_legacy_fields: false,
        }
    }
}

fn normalize_model_entry(
    id: &str,
    entry: &ProviderModelConfig,
    inherited_api_key: Option<&String>,
) -> RegisteredModel {
    let provider = entry
        .provider
        .clone()
        .unwrap_or_else(|| "openai".to_string())
        .to_ascii_lowercase();
    let protocol = entry
        .protocol
        .clone()
        .unwrap_or_else(|| infer_protocol(&provider).to_string())
        .to_ascii_lowercase();
    let model = entry
        .model
        .clone()
        .unwrap_or_else(|| default_model_name(&provider).to_string());
    let env_api_key = entry
        .api_key_env
        .as_ref()
        .and_then(|env_name| std::env::var(env_name).ok())
        .filter(|v| !v.trim().is_empty());
    let api_key = env_api_key
        .or_else(|| entry.api_key.clone())
        .or_else(|| inherited_api_key.cloned());

    RegisteredModel {
        id: id.to_string(),
        provider,
        protocol: protocol.clone(),
        model,
        request_profile: entry.request_profile.clone().or_else(|| {
            infer_request_profile(
                &entry
                    .provider
                    .clone()
                    .unwrap_or_else(|| "openai".to_string())
                    .to_ascii_lowercase(),
                &protocol,
                entry.base_url.as_deref(),
                entry.model.as_deref(),
            )
        }),
        base_url: entry.base_url.clone(),
        api_key,
        anthropic_version: entry.anthropic_version.clone(),
        thinking_budget_tokens: entry.thinking_budget_tokens,
        capabilities: CapabilityFlags {
            stream: entry
                .supports_stream
                .unwrap_or(default_capabilities(&protocol).stream),
            reasoning: entry
                .supports_reasoning
                .unwrap_or(default_capabilities(&protocol).reasoning),
            tool_call: entry
                .supports_tool_call
                .unwrap_or(default_capabilities(&protocol).tool_call),
            json_mode: entry
                .supports_json_mode
                .unwrap_or(default_capabilities(&protocol).json_mode),
        },
    }
}

fn infer_protocol(provider: &str) -> &'static str {
    match provider {
        "anthropic" | "glm" | "kimi" | "minimax" => "anthropic_messages",
        _ => "chat_completions",
    }
}

fn default_model_name(provider: &str) -> &'static str {
    match provider {
        "glm" => "GLM-4.7",
        "anthropic" => "claude-sonnet-4-5",
        "minimax" => "MiniMax-M2.7",
        _ => "gpt-4.1-mini",
    }
}

fn default_capabilities(protocol: &str) -> CapabilityFlags {
    match protocol {
        "anthropic_messages" => CapabilityFlags {
            stream: true,
            reasoning: true,
            tool_call: false,
            json_mode: false,
        },
        _ => CapabilityFlags {
            stream: true,
            reasoning: false,
            tool_call: false,
            json_mode: false,
        },
    }
}

fn infer_request_profile(
    provider: &str,
    protocol: &str,
    _base_url: Option<&str>,
    _model: Option<&str>,
) -> Option<String> {
    if protocol == "anthropic_messages" || provider.contains("anthropic") {
        return Some("anthropic_default".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{ModelConfig, ModelRoutingConfig, ProviderModelConfig};

    use super::ModelRegistry;

    #[test]
    fn bridges_legacy_single_model_config() {
        let config = ModelConfig {
            backend: Some("glm".to_string()),
            model: Some("GLM-4.7".to_string()),
            api_key: Some("legacy-key".to_string()),
            base_url: Some("https://open.bigmodel.cn/api/anthropic".to_string()),
            models: HashMap::new(),
            routing: None,
        };

        let registry = ModelRegistry::from_config(Some(&config));
        assert!(registry.used_legacy_bridge);
        assert!(registry.models.contains_key("default"));
        assert_eq!(registry.routing.planner, "default");
    }

    #[test]
    fn parses_new_config_with_routing() {
        let mut models = HashMap::new();
        models.insert(
            "glm_fast".to_string(),
            ProviderModelConfig {
                provider: Some("glm".to_string()),
                protocol: Some("anthropic_messages".to_string()),
                model: Some("GLM-4.7".to_string()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_string()),
                api_key: Some("test-key".to_string()),
                api_key_env: None,
                request_profile: None,
                anthropic_version: None,
                thinking_budget_tokens: Some(2000),
                supports_stream: Some(true),
                supports_reasoning: Some(true),
                supports_tool_call: Some(false),
                supports_json_mode: Some(false),
                context_window_size: None,
            },
        );
        let mut fallbacks = HashMap::new();
        fallbacks.insert("glm_fast".to_string(), vec!["openai_backup".to_string()]);
        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: Some(ModelRoutingConfig {
                planner: Some("glm_fast".to_string()),
                final_writer: None,
                tool_reasoning: None,
                fallbacks,
            }),
        };
        let registry = ModelRegistry::from_config(Some(&config));
        assert_eq!(registry.routing.planner, "glm_fast");
        assert_eq!(registry.routing.final_writer, "glm_fast");
        assert!(!registry.used_legacy_bridge);
    }

    #[test]
    fn resolves_api_key_from_api_key_env() {
        let var_name = "OPENJAX_TEST_REGISTRY_API_KEY";
        unsafe {
            std::env::set_var(var_name, "env-key");
        }

        let mut models = HashMap::new();
        models.insert(
            "m1".to_string(),
            ProviderModelConfig {
                provider: Some("glm".to_string()),
                protocol: Some("anthropic_messages".to_string()),
                model: Some("GLM-4.7".to_string()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_string()),
                api_key: None,
                api_key_env: Some(var_name.to_string()),
                request_profile: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                supports_stream: None,
                supports_reasoning: None,
                supports_tool_call: None,
                supports_json_mode: None,
                context_window_size: None,
            },
        );

        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: None,
        };

        let registry = ModelRegistry::from_config(Some(&config));
        assert_eq!(
            registry.models.get("m1").and_then(|m| m.api_key.as_deref()),
            Some("env-key")
        );
        unsafe {
            std::env::remove_var(var_name);
        }
    }

    #[test]
    fn api_key_env_takes_precedence_over_api_key() {
        let var_name = "OPENJAX_TEST_REGISTRY_API_KEY_PRIORITY";
        unsafe {
            std::env::set_var(var_name, "env-key-priority");
        }

        let mut models = HashMap::new();
        models.insert(
            "m1".to_string(),
            ProviderModelConfig {
                provider: Some("glm".to_string()),
                protocol: Some("anthropic_messages".to_string()),
                model: Some("GLM-4.7".to_string()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_string()),
                api_key: Some("inline-key".to_string()),
                api_key_env: Some(var_name.to_string()),
                request_profile: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                supports_stream: None,
                supports_reasoning: None,
                supports_tool_call: None,
                supports_json_mode: None,
                context_window_size: None,
            },
        );

        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: None,
        };

        let registry = ModelRegistry::from_config(Some(&config));
        assert_eq!(
            registry.models.get("m1").and_then(|m| m.api_key.as_deref()),
            Some("env-key-priority")
        );
        unsafe {
            std::env::remove_var(var_name);
        }
    }

    #[test]
    fn infers_kimi_request_profile_when_missing() {
        let mut models = HashMap::new();
        models.insert(
            "kimi".to_string(),
            ProviderModelConfig {
                provider: Some("kimi".to_string()),
                protocol: None,
                model: Some("kimi-k2".to_string()),
                base_url: Some("https://api.kimi.ai/anthropic/v1".to_string()),
                api_key: Some("test-key".to_string()),
                api_key_env: None,
                request_profile: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                supports_stream: None,
                supports_reasoning: None,
                supports_tool_call: None,
                supports_json_mode: None,
                context_window_size: None,
            },
        );

        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: None,
        };

        let registry = ModelRegistry::from_config(Some(&config));
        let kimi = registry.models.get("kimi").expect("kimi model registered");
        assert_eq!(kimi.protocol, "anthropic_messages");
        assert_eq!(kimi.request_profile.as_deref(), Some("anthropic_default"));
    }

    #[test]
    fn infers_anthropic_request_profile_when_missing() {
        let mut models = HashMap::new();
        models.insert(
            "claude".to_string(),
            ProviderModelConfig {
                provider: Some("anthropic".to_string()),
                protocol: Some("anthropic_messages".to_string()),
                model: Some("claude-sonnet-4-6".to_string()),
                base_url: Some("https://api.anthropic.com".to_string()),
                api_key: Some("test-key".to_string()),
                api_key_env: None,
                request_profile: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                supports_stream: None,
                supports_reasoning: None,
                supports_tool_call: None,
                supports_json_mode: None,
                context_window_size: None,
            },
        );

        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: None,
        };

        let registry = ModelRegistry::from_config(Some(&config));
        assert_eq!(
            registry
                .models
                .get("claude")
                .and_then(|model| model.request_profile.as_deref()),
            Some("anthropic_default")
        );
    }
}
