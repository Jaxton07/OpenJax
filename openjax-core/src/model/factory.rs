use crate::config::ModelConfig;
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
        Some("glm") => {
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
}
