use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::config::ModelConfig;

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, user_input: &str) -> Result<String>;

    fn name(&self) -> &'static str;
}

#[derive(Debug, Default)]
pub struct EchoModelClient;

#[async_trait]
impl ModelClient for EchoModelClient {
    async fn complete(&self, user_input: &str) -> Result<String> {
        Ok(format!("[Echo fallback] {user_input}"))
    }

    fn name(&self) -> &'static str {
        "echo"
    }
}

#[derive(Debug, Clone)]
pub struct ChatCompletionsClient {
    client: Client,
    api_key: String,
    model: String,
    endpoint: String,
    backend_name: &'static str,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

impl ChatCompletionsClient {
    pub fn from_minimax_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENJAX_MINIMAX_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());
        
        let config_api_key = config.and_then(|c| c.api_key.as_ref());
        
        let api_key = env_api_key
            .or_else(|| config_api_key.map(|s| s.clone()))?;
        
        let model = std::env::var("OPENJAX_MINIMAX_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.model.clone()))
            .unwrap_or_else(|| "codex-MiniMax-M2.1".to_string());
        
        let base_url = std::env::var("OPENJAX_MINIMAX_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.base_url.clone()))
            .unwrap_or_else(|| "https://api.minimaxi.com/v1".to_string());
        
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint,
            backend_name: "minimax-chat-completions",
        })
    }

    pub fn from_openai_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());
        
        let config_api_key = config.and_then(|c| c.api_key.as_ref());
        
        let api_key = env_api_key
            .or_else(|| config_api_key.map(|s| s.clone()))?;
        
        let model = std::env::var("OPENJAX_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.model.clone()))
            .unwrap_or_else(|| "gpt-4.1-mini".to_string());
        
        let base_url = std::env::var("OPENAI_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.base_url.clone()))
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint,
            backend_name: "openai-chat-completions",
        })
    }

    pub fn from_glm_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENJAX_GLM_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let config_api_key = config.and_then(|c| c.api_key.as_ref());

        let api_key = env_api_key
            .or_else(|| config_api_key.map(|s| s.clone()))?;

        let model = std::env::var("OPENJAX_GLM_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.model.clone()))
            .unwrap_or_else(|| "GLM-4.7".to_string());

        let base_url = std::env::var("OPENJAX_GLM_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.base_url.clone()))
            .unwrap_or_else(|| "https://open.bigmodel.cn/api/coding/paas/v4".to_string());

        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint,
            backend_name: "glm-chat-completions",
        })
    }
}

fn response_snippet(body: &str) -> String {
    let max = 400;
    if body.chars().count() <= max {
        body.to_string()
    } else {
        let snippet = body.chars().take(max).collect::<String>();
        format!("{snippet}...")
    }
}

fn extract_content_from_body(body: &serde_json::Value) -> Option<String> {
    let message = body
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("message"))?;

    let content = message.get("content")?;

    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    // Some providers return content blocks like:
    // [{"type":"text","text":"..."}, ...]
    if let Some(blocks) = content.as_array() {
        let mut merged = String::new();
        for block in blocks {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                if !merged.is_empty() {
                    merged.push('\n');
                }
                merged.push_str(text);
            }
        }
        if !merged.is_empty() {
            return Some(merged);
        }
    }

    None
}

#[async_trait]
impl ModelClient for ChatCompletionsClient {
    async fn complete(&self, user_input: &str) -> Result<String> {
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are OpenJax, a pragmatic coding assistant in terminal CLI. Keep responses concise.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_input.to_string(),
                },
            ],
            temperature: 0.2,
        };

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .header("accept", "application/json")
            .json(&req)
            .send()
            .await
            .context("chat completions request failed")?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .context("failed to read chat completions response body")?;

        let body_json: serde_json::Value = serde_json::from_str(&body_text).map_err(|err| {
            anyhow!(
                "failed to parse chat completions response JSON: {err}; status={status}; body_snippet={}",
                response_snippet(&body_text)
            )
        })?;

        if !status.is_success() {
            return Err(anyhow!(
                "chat completions API error ({status}): {}",
                response_snippet(&body_text)
            ));
        }

        let content = extract_content_from_body(&body_json).ok_or_else(|| {
            anyhow!(
                "missing choices[0].message.content in API response; status={status}; body_snippet={}",
                response_snippet(&body_text)
            )
        })?;

        Ok(content)
    }

    fn name(&self) -> &'static str {
        self.backend_name
    }
}

pub fn build_model_client() -> Box<dyn ModelClient> {
    build_model_client_with_config(None)
}

pub fn build_model_client_with_config(config: Option<&ModelConfig>) -> Box<dyn ModelClient> {
    let backend = config.and_then(|c| c.backend.as_ref()).map(|s| s.to_lowercase());
    
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
