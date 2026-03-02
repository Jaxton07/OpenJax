use std::sync::Arc;

use tracing::warn;

use crate::config::ModelConfig;
use crate::model::anthropic_messages::AnthropicMessagesClient;
use crate::model::chat_completions::ChatCompletionsClient;
use crate::model::client::{ModelClient, ProviderAdapter};
use crate::model::echo::EchoModelClient;
use crate::model::missing_config::MissingConfigModelClient;
use crate::model::registry::ModelRegistry;
use crate::model::router::ModelRouter;

pub fn build_model_client() -> Box<dyn ModelClient> {
    build_model_client_with_config(None)
}

pub fn build_model_client_with_config(config: Option<&ModelConfig>) -> Box<dyn ModelClient> {
    if config
        .and_then(|c| c.backend.as_deref())
        .is_some_and(|backend| backend.eq_ignore_ascii_case("echo"))
    {
        return Box::new(EchoModelClient);
    }

    let registry = ModelRegistry::from_config(config);

    if registry.used_legacy_bridge && !registry.models.is_empty() {
        warn!("model config uses legacy [model] fields; bridged to [model.models.default]");
    }
    if registry.has_legacy_fields && !registry.models.is_empty() {
        warn!("both legacy [model] and new [model.models] detected; new config takes precedence");
    }

    let mut adapters: Vec<Arc<dyn ProviderAdapter>> = Vec::new();
    for model in registry.models.values() {
        if let Some(adapter) = build_adapter_for_registered_model(model) {
            adapters.push(adapter);
        } else {
            warn!(
                model_id = %model.id,
                provider = %model.provider,
                protocol = %model.protocol,
                "unable to build provider adapter for model entry"
            );
        }
    }

    if adapters.is_empty() {
        if let Some(client) = ChatCompletionsClient::from_minimax_config(config) {
            return Box::new(client);
        }
        if let Some(client) = AnthropicMessagesClient::from_anthropic_config(config) {
            return Box::new(client);
        }
        if let Some(client) = AnthropicMessagesClient::from_glm_config(config) {
            return Box::new(client);
        }
        if let Some(client) = ChatCompletionsClient::from_openai_config(config) {
            return Box::new(client);
        }
        if let Some(client) = ChatCompletionsClient::from_glm_config(config) {
            return Box::new(client);
        }
        return Box::new(MissingConfigModelClient::new(
            "未检测到可用模型配置。请在 .openjax/config/config.toml 配置 model.models，并设置对应 API Key 环境变量（如 OPENJAX_GLM_API_KEY / OPENAI_API_KEY）。",
        ));
    }

    Box::new(ModelRouter::new(registry, adapters))
}

fn build_adapter_for_registered_model(
    model: &crate::model::registry::RegisteredModel,
) -> Option<Arc<dyn ProviderAdapter>> {
    match model.protocol.as_str() {
        "anthropic_messages" => AnthropicMessagesClient::from_registered_model(model)
            .map(|c| Arc::new(c) as Arc<dyn ProviderAdapter>),
        "chat_completions" => ChatCompletionsClient::from_registered_model(model)
            .map(|c| Arc::new(c) as Arc<dyn ProviderAdapter>),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ModelConfig, ModelRoutingConfig, ProviderModelConfig};
    use crate::model::factory::build_model_client_with_config;
    use std::collections::HashMap;

    #[test]
    fn build_model_client_respects_echo_backend() {
        let config = ModelConfig {
            backend: Some("echo".to_string()),
            model: None,
            api_key: None,
            base_url: None,
            models: HashMap::new(),
            routing: None,
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "echo");
    }

    #[test]
    fn build_model_client_returns_missing_config_client_when_no_provider_available() {
        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models: HashMap::new(),
            routing: None,
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "missing-model-config");
    }

    #[test]
    fn build_model_client_respects_new_registry_config() {
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
                anthropic_version: None,
                thinking_budget_tokens: Some(2000),
                supports_stream: Some(true),
                supports_reasoning: Some(true),
                supports_tool_call: Some(false),
                supports_json_mode: Some(false),
            },
        );
        let config = ModelConfig {
            backend: None,
            model: None,
            api_key: None,
            base_url: None,
            models,
            routing: Some(ModelRoutingConfig {
                planner: Some("glm_fast".to_string()),
                final_writer: Some("glm_fast".to_string()),
                tool_reasoning: Some("glm_fast".to_string()),
                fallbacks: HashMap::new(),
            }),
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "model-router");
    }

    #[test]
    fn build_model_client_still_supports_legacy_openai_config() {
        let config = ModelConfig {
            backend: Some("openai".to_string()),
            model: Some("gpt-4.1-mini".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            models: HashMap::new(),
            routing: None,
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "model-router");
    }
}
