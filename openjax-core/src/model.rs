use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

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
    pub fn from_minimax_env() -> Option<Self> {
        let api_key = std::env::var("OPENJAX_MINIMAX_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())?;
        let model = std::env::var("OPENJAX_MINIMAX_MODEL")
            .unwrap_or_else(|_| "codex-MiniMax-M2.1".to_string());
        let base_url = std::env::var("OPENJAX_MINIMAX_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimaxi.com/v1".to_string());
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint,
            backend_name: "minimax-chat-completions",
        })
    }

    pub fn from_openai_env() -> Option<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())?;
        let model = std::env::var("OPENJAX_MODEL").unwrap_or_else(|_| "gpt-4.1-mini".to_string());
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint,
            backend_name: "openai-chat-completions",
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
    if let Some(client) = ChatCompletionsClient::from_minimax_env() {
        return Box::new(client);
    }

    if let Some(client) = ChatCompletionsClient::from_openai_env() {
        return Box::new(client);
    }

    Box::new(EchoModelClient)
}
