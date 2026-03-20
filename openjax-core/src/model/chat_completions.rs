use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

use crate::config::ModelConfig;
use crate::logger::{RAW_RESPONSE_LOG_TARGET, provider_raw_log_enabled};
use crate::model::client::{ModelClient, ProviderAdapter};
use crate::model::registry::RegisteredModel;
use crate::model::types::{CapabilityFlags, ModelRequest, ModelResponse, ModelUsage, StreamDelta};
use crate::streaming::parser::{SseParser, openai::OpenAiSseParser};

const SYSTEM_PROMPT_PERSONA: &str = "You are OpenJax, an all-purpose personal AI assistant in a terminal environment, similar in spirit to a reliable AI butler.";
const SYSTEM_PROMPT_BEHAVIOR: &str = "Your job is to help the user get outcomes across many domains: system and environment checks, document and knowledge tasks, coding and debugging, shell workflows, planning, and everyday productivity. \
Be practical, accurate, and action-oriented. Prefer using available tools when verification or execution is needed. \
Keep responses concise, clear, and directly useful.";
const SYSTEM_PROMPT_SAFETY: &str =
    "For high-impact actions, surface assumptions and confirm intent before proceeding.";
const STREAM_IDLE_TIMEOUT_SECS: u64 = 300;

fn default_system_prompt() -> String {
    format!(
        "{}\n\nBehavior guidelines:\n{}\n\nSafety boundaries:\n{}",
        SYSTEM_PROMPT_PERSONA, SYSTEM_PROMPT_BEHAVIOR, SYSTEM_PROMPT_SAFETY
    )
}

#[derive(Debug, Clone)]
pub(crate) struct ChatCompletionsClient {
    client: Client,
    model_id: String,
    provider: String,
    protocol: String,
    api_key: String,
    model: String,
    endpoint: String,
    backend_name: &'static str,
    capabilities: CapabilityFlags,
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

impl ChatCompletionsClient {
    pub(crate) fn from_registered_model(entry: &RegisteredModel) -> Option<Self> {
        if entry.protocol != "chat_completions" {
            return None;
        }
        let api_key = entry
            .api_key
            .clone()
            .or_else(|| default_api_key_for_provider(&entry.provider))?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url_for_provider(&entry.provider).to_string());
        Some(Self {
            client: Client::new(),
            model_id: entry.id.clone(),
            provider: entry.provider.clone(),
            protocol: entry.protocol.clone(),
            api_key,
            model: entry.model.clone(),
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
            backend_name: "chat-completions",
            capabilities: entry.capabilities,
        })
    }

    pub(crate) fn from_minimax_config(config: Option<&ModelConfig>) -> Option<Self> {
        Self::from_provider_config(
            config,
            "OPENJAX_MINIMAX_API_KEY",
            "OPENJAX_MINIMAX_MODEL",
            "OPENJAX_MINIMAX_BASE_URL",
            "codex-MiniMax-M2.1",
            "https://api.minimaxi.com/v1",
            "minimax-chat-completions",
        )
    }

    pub(crate) fn from_openai_config(config: Option<&ModelConfig>) -> Option<Self> {
        Self::from_provider_config(
            config,
            "OPENAI_API_KEY",
            "OPENJAX_MODEL",
            "OPENAI_BASE_URL",
            "gpt-4.1-mini",
            "https://api.openai.com/v1",
            "openai-chat-completions",
        )
    }

    pub(crate) fn from_glm_config(config: Option<&ModelConfig>) -> Option<Self> {
        Self::from_provider_config(
            config,
            "OPENJAX_GLM_API_KEY",
            "OPENJAX_GLM_MODEL",
            "OPENJAX_GLM_BASE_URL",
            "GLM-4.7",
            "https://open.bigmodel.cn/api/coding/paas/v4",
            "glm-chat-completions",
        )
    }

    fn from_provider_config(
        config: Option<&ModelConfig>,
        api_key_env: &str,
        model_env: &str,
        base_url_env: &str,
        default_model: &str,
        default_base_url: &str,
        backend_name: &'static str,
    ) -> Option<Self> {
        let env_api_key = std::env::var(api_key_env)
            .ok()
            .filter(|v| !v.trim().is_empty());
        let config_api_key = config.and_then(|c| c.api_key.as_ref());
        let api_key = env_api_key.or_else(|| config_api_key.cloned())?;

        let model = std::env::var(model_env)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.model.clone()))
            .unwrap_or_else(|| default_model.to_string());

        let base_url = std::env::var(base_url_env)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| config.and_then(|c| c.base_url.clone()))
            .unwrap_or_else(|| default_base_url.to_string());

        let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        Some(Self {
            client: Client::new(),
            model_id: backend_name.to_string(),
            provider: backend_name
                .split('-')
                .next()
                .unwrap_or("openai")
                .to_string(),
            protocol: "chat_completions".to_string(),
            api_key,
            model,
            endpoint,
            backend_name,
            capabilities: CapabilityFlags {
                stream: true,
                reasoning: false,
                tool_call: false,
                json_mode: false,
            },
        })
    }
}

fn default_api_key_for_provider(provider: &str) -> Option<String> {
    let key = match provider {
        "minimax" => "OPENJAX_MINIMAX_API_KEY",
        "glm" => "OPENJAX_GLM_API_KEY",
        _ => "OPENAI_API_KEY",
    };
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn default_base_url_for_provider(provider: &str) -> &'static str {
    match provider {
        "minimax" => "https://api.minimaxi.com/v1",
        "glm" => "https://open.bigmodel.cn/api/coding/paas/v4",
        _ => "https://api.openai.com/v1",
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

static MODEL_STREAM_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

fn model_stream_debug_enabled() -> bool {
    *MODEL_STREAM_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("OPENJAX_MODEL_STREAM_DEBUG")
            .ok()
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "0" | "off" | "false" | "disabled"
                )
            })
            .unwrap_or(false)
    })
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

fn extract_delta_reasoning_from_body(body: &serde_json::Value) -> Option<String> {
    body.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("delta"))
        .and_then(|v| v.get("reasoning_content"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

#[async_trait]
impl ModelClient for ChatCompletionsClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: request
                        .system_prompt
                        .clone()
                        .unwrap_or_else(default_system_prompt),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: request.user_input.to_string(),
                },
            ],
            temperature: 0.2,
            stream: None,
            stream_options: None,
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

        let usage = body_json.get("usage").map(|usage| ModelUsage {
            input_tokens: usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64())),
            output_tokens: usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64())),
            total_tokens: usage.get("total_tokens").and_then(|v| v.as_u64()),
        });

        let finish_reason = body_json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("finish_reason"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        Ok(ModelResponse {
            text: content,
            reasoning: None,
            usage,
            finish_reason,
            raw: Some(body_json),
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: request
                        .system_prompt
                        .clone()
                        .unwrap_or_else(default_system_prompt),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: request.user_input.to_string(),
                },
            ],
            temperature: 0.2,
            stream: Some(true),
            stream_options: Some(StreamOptions { include_usage: true }),
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
        let mut last_usage: Option<ModelUsage> = None;
        let mut parser = OpenAiSseParser::default();
        let stream_debug = model_stream_debug_enabled();
        let stream_started_at = Instant::now();
        let mut chunk_seq = 0u64;
        let mut frame_seq = 0u64;
        let mut delta_seq = 0u64;
        let mut last_delta_at: Option<Instant> = None;
        let mut last_chunk_at: Option<Instant> = None;
        let raw_log_enabled = provider_raw_log_enabled();

        let mut resp = resp;
        loop {
            let maybe_chunk = tokio::time::timeout(
                Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
                resp.chunk(),
            )
            .await
            .map_err(|_| {
                anyhow!(
                    "chat completions stream timed out after {}s waiting for next chunk before [DONE]",
                    STREAM_IDLE_TIMEOUT_SECS
                )
            })?;
            let Some(chunk) = maybe_chunk.context("failed reading stream chunk")? else {
                break;
            };
            chunk_seq += 1;
            last_chunk_at = Some(Instant::now());
            if raw_log_enabled {
                let raw_chunk = String::from_utf8_lossy(&chunk).into_owned();
                info!(
                    target: RAW_RESPONSE_LOG_TARGET,
                    backend = self.backend_name,
                    model_id = %self.model_id,
                    stage = request.stage.as_str(),
                    chunk_seq = chunk_seq,
                    chunk_bytes = chunk.len(),
                    raw_chunk = %raw_chunk,
                    "provider_stream_raw_chunk"
                );
            }
            if stream_debug {
                debug!(
                    backend = self.backend_name,
                    model_id = %self.model_id,
                    stage = request.stage.as_str(),
                    chunk_seq = chunk_seq,
                    chunk_bytes = chunk.len(),
                    "model_stream_chunk_received"
                );
            }
            for frame in parser.push_chunk(&chunk)? {
                frame_seq += 1;
                let payload: serde_json::Value = serde_json::from_str(&frame).map_err(|err| {
                    anyhow!(
                        "failed to parse SSE JSON chunk: {err}; chunk_snippet={}",
                        response_snippet(&frame)
                    )
                })?;

                if let Some(delta) = extract_delta_content_from_body(&payload)
                    && !delta.is_empty()
                {
                    let delta_len = delta.chars().count();
                    delta_seq += 1;
                    last_delta_at = Some(Instant::now());
                    assembled.push_str(&delta);
                    if let Some(sender) = &delta_sender {
                        let _ = sender.send(StreamDelta::Text(delta));
                    }
                    if stream_debug {
                        debug!(
                            backend = self.backend_name,
                            model_id = %self.model_id,
                            stage = request.stage.as_str(),
                            frame_seq = frame_seq,
                            delta_seq = delta_seq,
                            delta_len = delta_len,
                            assembled_len = assembled.len(),
                            "model_stream_delta_emitted"
                        );
                    }
                } else if stream_debug {
                    debug!(
                        backend = self.backend_name,
                        model_id = %self.model_id,
                        stage = request.stage.as_str(),
                        frame_seq = frame_seq,
                        "model_stream_frame_without_text_delta"
                    );
                }

                if let Some(reasoning_delta) = extract_delta_reasoning_from_body(&payload)
                    && !reasoning_delta.is_empty()
                    && let Some(sender) = &delta_sender
                {
                    let _ = sender.send(StreamDelta::Reasoning(reasoning_delta));
                }

                if let Some(usage_val) = payload.get("usage") {
                    last_usage = Some(ModelUsage {
                        input_tokens: usage_val
                            .get("prompt_tokens")
                            .and_then(|v| v.as_u64())
                            .or_else(|| usage_val.get("input_tokens").and_then(|v| v.as_u64())),
                        output_tokens: usage_val
                            .get("completion_tokens")
                            .and_then(|v| v.as_u64())
                            .or_else(|| usage_val.get("output_tokens").and_then(|v| v.as_u64())),
                        total_tokens: usage_val.get("total_tokens").and_then(|v| v.as_u64()),
                    });
                }
            }
        }

        for frame in parser.finish()? {
            frame_seq += 1;
            let payload: serde_json::Value = serde_json::from_str(&frame).map_err(|err| {
                anyhow!(
                    "failed to parse trailing SSE JSON chunk: {err}; chunk_snippet={}",
                    response_snippet(&frame)
                )
            })?;
            if let Some(delta) = extract_delta_content_from_body(&payload)
                && !delta.is_empty()
            {
                let delta_len = delta.chars().count();
                delta_seq += 1;
                last_delta_at = Some(Instant::now());
                assembled.push_str(&delta);
                if let Some(sender) = &delta_sender {
                    let _ = sender.send(StreamDelta::Text(delta));
                }
                if stream_debug {
                    debug!(
                        backend = self.backend_name,
                        model_id = %self.model_id,
                        stage = request.stage.as_str(),
                        frame_seq = frame_seq,
                        delta_seq = delta_seq,
                        delta_len = delta_len,
                        assembled_len = assembled.len(),
                        "model_stream_delta_emitted"
                    );
                }
            }

            if let Some(reasoning_delta) = extract_delta_reasoning_from_body(&payload)
                && !reasoning_delta.is_empty()
                && let Some(sender) = &delta_sender
            {
                let _ = sender.send(StreamDelta::Reasoning(reasoning_delta));
            }

            if let Some(usage_val) = payload.get("usage") {
                last_usage = Some(ModelUsage {
                    input_tokens: usage_val
                        .get("prompt_tokens")
                        .and_then(|v| v.as_u64())
                        .or_else(|| usage_val.get("input_tokens").and_then(|v| v.as_u64())),
                    output_tokens: usage_val
                        .get("completion_tokens")
                        .and_then(|v| v.as_u64())
                        .or_else(|| usage_val.get("output_tokens").and_then(|v| v.as_u64())),
                    total_tokens: usage_val.get("total_tokens").and_then(|v| v.as_u64()),
                });
            }
        }

        if stream_debug {
            debug!(
                backend = self.backend_name,
                model_id = %self.model_id,
                stage = request.stage.as_str(),
                stream_total_ms = stream_started_at.elapsed().as_millis() as u64,
                chunk_count = chunk_seq,
                frame_count = frame_seq,
                delta_count = delta_seq,
                assembled_len = assembled.len(),
                tail_silence_ms = last_delta_at
                    .map(|ts| ts.elapsed().as_millis() as u64)
                    .unwrap_or(stream_started_at.elapsed().as_millis() as u64),
                "model_stream_summary"
            );
        }

        let ended_by_eof = true;
        let last_chunk_gap_ms = last_chunk_at
            .map(|ts| ts.elapsed().as_millis() as u64)
            .unwrap_or(stream_started_at.elapsed().as_millis() as u64);
        info!(
            backend = self.backend_name,
            model_id = %self.model_id,
            stage = request.stage.as_str(),
            done_seen = parser.saw_done_marker(),
            ended_by_eof = ended_by_eof,
            chunk_count = chunk_seq,
            frame_count = frame_seq,
            last_chunk_gap_ms = last_chunk_gap_ms,
            "model_stream_done_check"
        );

        if !parser.saw_done_marker() {
            warn!(
                backend = self.backend_name,
                model_id = %self.model_id,
                stage = request.stage.as_str(),
                ended_by_eof = ended_by_eof,
                chunk_count = chunk_seq,
                frame_count = frame_seq,
                last_chunk_gap_ms = last_chunk_gap_ms,
                "model_stream_done_missing"
            );
            return Err(anyhow!(
                "chat completions stream ended before [DONE]; treating as protocol error"
            ));
        }

        if assembled.is_empty() {
            return Err(anyhow!(
                "missing streaming delta content in API response; status={status}"
            ));
        }

        Ok(ModelResponse {
            text: assembled,
            reasoning: None,
            usage: last_usage,
            finish_reason: None,
            raw: None,
        })
    }

    fn name(&self) -> &'static str {
        self.backend_name
    }
}

#[async_trait]
impl ProviderAdapter for ChatCompletionsClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        <Self as ModelClient>::complete(self, request).await
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        <Self as ModelClient>::complete_stream(self, request, delta_sender).await
    }

    fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        &self.provider
    }

    fn protocol(&self) -> &str {
        &self.protocol
    }

    fn capabilities(&self) -> CapabilityFlags {
        self.capabilities
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        extract_content_from_body, extract_delta_content_from_body,
        extract_delta_reasoning_from_body,
    };
    use crate::streaming::parser::parse_sse_data_line;

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

    #[test]
    fn extract_delta_reasoning_supports_reasoning_content() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "reasoning_content": "step-by-step"
                    }
                }
            ]
        });
        let reasoning = extract_delta_reasoning_from_body(&body);
        assert_eq!(reasoning.as_deref(), Some("step-by-step"));
    }

    #[test]
    fn extract_delta_supports_mixed_content_and_reasoning() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "content": "answer",
                        "reasoning_content": "thinking"
                    }
                }
            ]
        });
        assert_eq!(
            extract_delta_content_from_body(&body).as_deref(),
            Some("answer")
        );
        assert_eq!(
            extract_delta_reasoning_from_body(&body).as_deref(),
            Some("thinking")
        );
    }
}

#[cfg(test)]
mod streaming_usage_tests {
    use crate::model::types::ModelUsage;

    #[test]
    fn test_usage_extracted_from_last_sse_frame() {
        // Simulate the last SSE chunk containing usage (OpenAI format)
        let frame = r#"{"id":"test","choices":[{"index":0,"finish_reason":"stop","delta":{"content":""}}],"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}"#;
        let val: serde_json::Value = serde_json::from_str(frame).unwrap();
        let usage_val = val.get("usage").unwrap();
        let usage = ModelUsage {
            input_tokens: usage_val.get("prompt_tokens").and_then(|v| v.as_u64()),
            output_tokens: usage_val.get("completion_tokens").and_then(|v| v.as_u64()),
            total_tokens: usage_val.get("total_tokens").and_then(|v| v.as_u64()),
        };
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.total_tokens, Some(150));
    }

    #[test]
    fn test_usage_extracted_from_glm_format() {
        // Simulate GLM-style usage with input_tokens / output_tokens keys
        let frame = r#"{"id":"test","choices":[{"index":0,"finish_reason":"stop","delta":{"content":""}}],"usage":{"input_tokens":80,"output_tokens":40,"total_tokens":120}}"#;
        let val: serde_json::Value = serde_json::from_str(frame).unwrap();
        let usage_val = val.get("usage").unwrap();
        let usage = ModelUsage {
            input_tokens: usage_val
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .or_else(|| usage_val.get("input_tokens").and_then(|v| v.as_u64())),
            output_tokens: usage_val
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .or_else(|| usage_val.get("output_tokens").and_then(|v| v.as_u64())),
            total_tokens: usage_val.get("total_tokens").and_then(|v| v.as_u64()),
        };
        assert_eq!(usage.input_tokens, Some(80));
        assert_eq!(usage.output_tokens, Some(40));
        assert_eq!(usage.total_tokens, Some(120));
    }
}
