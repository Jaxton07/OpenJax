use crate::config::ModelConfig;
use crate::model::anthropic_messages::AnthropicMessagesClient;
use crate::model::chat_completions::ChatCompletionsClient;
use crate::model::client::ModelClient;
use crate::model::echo::EchoModelClient;

pub fn build_model_client() -> Box<dyn ModelClient> {
    build_model_client_with_config(None)
}

pub fn build_model_client_with_config(config: Option<&ModelConfig>) -> Box<dyn ModelClient> {
    let backend = config
        .and_then(|c| c.backend.as_ref())
        .map(|s| s.to_lowercase());

    match backend.as_deref() {
        Some("anthropic") => {
            if let Some(client) = AnthropicMessagesClient::from_anthropic_config(config) {
                return Box::new(client);
            }
        }
        Some("glm") => {
            if let Some(client) = AnthropicMessagesClient::from_glm_config(config) {
                return Box::new(client);
            }
            if let Some(client) = ChatCompletionsClient::from_glm_config(config) {
                return Box::new(client);
            }
        }
        Some("minimax") => {
            if let Some(client) = ChatCompletionsClient::from_minimax_config(config) {
                return Box::new(client);
            }
        }
        Some("openai") => {
            if let Some(client) = ChatCompletionsClient::from_openai_config(config) {
                return Box::new(client);
            }
        }
        Some("echo") => {
            return Box::new(EchoModelClient);
        }
        _ => {}
    }

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

    Box::new(EchoModelClient)
}

#[cfg(test)]
mod tests {
    use crate::config::ModelConfig;
    use crate::model::factory::build_model_client_with_config;

    #[test]
    fn build_model_client_respects_echo_backend() {
        let config = ModelConfig {
            backend: Some("echo".to_string()),
            model: None,
            api_key: None,
            base_url: None,
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "echo");
    }

    #[test]
    fn build_model_client_respects_openai_backend_with_config_api_key() {
        let config = ModelConfig {
            backend: Some("openai".to_string()),
            model: Some("gpt-4.1-mini".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "openai-chat-completions");
    }

    #[test]
    fn build_model_client_respects_minimax_backend_with_config_api_key() {
        let config = ModelConfig {
            backend: Some("minimax".to_string()),
            model: Some("codex-MiniMax-M2.1".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://api.minimaxi.com/v1".to_string()),
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "minimax-chat-completions");
    }

    #[test]
    fn build_model_client_respects_anthropic_backend_with_config_api_key() {
        let config = ModelConfig {
            backend: Some("anthropic".to_string()),
            model: Some("claude-sonnet-4-5".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://api.anthropic.com/v1".to_string()),
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "anthropic-messages");
    }

    #[test]
    fn build_model_client_prefers_glm_anthropic_messages() {
        let config = ModelConfig {
            backend: Some("glm".to_string()),
            model: Some("GLM-4.7".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://open.bigmodel.cn/api/anthropic/v1".to_string()),
        };

        let client = build_model_client_with_config(Some(&config));
        assert_eq!(client.name(), "glm-anthropic-messages");
    }
}
