use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::ModelConfig;
use crate::model::client::ModelClient;

#[derive(Debug, Clone)]
pub(crate) struct ChatCompletionsClient {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

impl ChatCompletionsClient {
    pub(crate) fn from_minimax_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENJAX_MINIMAX_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let config_api_key = config.and_then(|c| c.api_key.as_ref());

        let api_key = env_api_key.or_else(|| config_api_key.cloned())?;

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

    pub(crate) fn from_openai_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let config_api_key = config.and_then(|c| c.api_key.as_ref());

        let api_key = env_api_key.or_else(|| config_api_key.cloned())?;

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

    pub(crate) fn from_glm_config(config: Option<&ModelConfig>) -> Option<Self> {
        let env_api_key = std::env::var("OPENJAX_GLM_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let config_api_key = config.and_then(|c| c.api_key.as_ref());

        let api_key = env_api_key.or_else(|| config_api_key.cloned())?;

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

fn extract_delta_content_from_body(body: &serde_json::Value) -> Option<String> {
    let delta = body
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("delta"))?;

    let content = delta.get("content")?;
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    if let Some(blocks) = content.as_array() {
        let mut merged = String::new();
        for block in blocks {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                merged.push_str(text);
            }
        }
        if !merged.is_empty() {
            return Some(merged);
        }
    }

    None
}

fn parse_sse_data_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with("data:") {
        return None;
    }
    Some(trimmed[5..].trim())
}

#[async_trait]
impl ModelClient for ChatCompletionsClient {
    async fn complete(&self, user_input: &str) -> Result<String> {
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content:
                        "You are OpenJax, a pragmatic coding assistant in terminal CLI. Keep responses concise."
                            .to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_input.to_string(),
                },
            ],
            temperature: 0.2,
            stream: None,
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

    async fn complete_stream(
        &self,
        user_input: &str,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<String> {
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content:
                        "You are OpenJax, a pragmatic coding assistant in terminal CLI. Keep responses concise."
                            .to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_input.to_string(),
                },
            ],
            temperature: 0.2,
            stream: Some(true),
        };

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .header("accept", "text/event-stream")
            .json(&req)
            .send()
            .await
            .context("chat completions streaming request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .context("failed to read streaming error response body")?;
            return Err(anyhow!(
                "chat completions streaming API error ({status}): {}",
                response_snippet(&body_text)
            ));
        }

        let mut assembled = String::new();
        let mut pending: Vec<u8> = Vec::new();

        let mut resp = resp;
        while let Some(chunk) = resp.chunk().await.context("failed reading stream chunk")? {
            pending.extend_from_slice(&chunk);

            while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                let mut line = pending.drain(..=pos).collect::<Vec<u8>>();
                if matches!(line.last(), Some(b'\n')) {
                    let _ = line.pop();
                }
                if matches!(line.last(), Some(b'\r')) {
                    let _ = line.pop();
                }

                let line_text = String::from_utf8_lossy(&line);
                let Some(data) = parse_sse_data_line(&line_text) else {
                    continue;
                };

                if data == "[DONE]" {
                    continue;
                }

                let payload: serde_json::Value = serde_json::from_str(data).map_err(|err| {
                    anyhow!(
                        "failed to parse SSE JSON chunk: {err}; chunk_snippet={}",
                        response_snippet(data)
                    )
                })?;

                if let Some(delta) = extract_delta_content_from_body(&payload)
                    && !delta.is_empty()
                {
                    assembled.push_str(&delta);
                    if let Some(sender) = &delta_sender {
                        let _ = sender.send(delta);
                    }
                }
            }
        }

        if !pending.is_empty() {
            let line_text = String::from_utf8_lossy(&pending);
            if let Some(data) = parse_sse_data_line(&line_text)
                && data != "[DONE]"
            {
                let payload: serde_json::Value = serde_json::from_str(data).map_err(|err| {
                    anyhow!(
                        "failed to parse trailing SSE JSON chunk: {err}; chunk_snippet={}",
                        response_snippet(data)
                    )
                })?;
                if let Some(delta) = extract_delta_content_from_body(&payload)
                    && !delta.is_empty()
                {
                    assembled.push_str(&delta);
                    if let Some(sender) = &delta_sender {
                        let _ = sender.send(delta);
                    }
                }
            }
        }

        if assembled.is_empty() {
            return Err(anyhow!(
                "missing streaming delta content in API response; status={status}"
            ));
        }

        Ok(assembled)
    }

    fn name(&self) -> &'static str {
        self.backend_name
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{extract_content_from_body, extract_delta_content_from_body, parse_sse_data_line};

    #[test]
    fn extract_content_supports_block_array() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            {"type": "text", "text": "hello"},
                            {"type": "text", "text": "world"}
                        ]
                    }
                }
            ]
        });

        let content = extract_content_from_body(&body);
        assert_eq!(content.as_deref(), Some("hello\nworld"));
    }

    #[test]
    fn extract_delta_supports_block_array() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "content": [
                            {"type": "text", "text": "he"},
                            {"type": "text", "text": "llo"}
                        ]
                    }
                }
            ]
        });

        let content = extract_delta_content_from_body(&body);
        assert_eq!(content.as_deref(), Some("hello"));
    }

    #[test]
    fn parse_sse_data_line_ignores_non_data_lines() {
        assert_eq!(parse_sse_data_line("event: ping"), None);
        assert_eq!(parse_sse_data_line("data: {\"x\":1}"), Some("{\"x\":1}"));
    }
}
