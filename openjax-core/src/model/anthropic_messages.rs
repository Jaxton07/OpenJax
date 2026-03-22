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
use crate::model::request_profiles::anthropic_messages::AnthropicMessagesRequestProfile;
use crate::model::types::{CapabilityFlags, ModelRequest, ModelResponse, ModelUsage, StreamDelta};
use crate::streaming::parser::{SseParser, anthropic::AnthropicSseParser};

const SYSTEM_PROMPT_PERSONA: &str = "You are OpenJax, an all-purpose personal AI assistant in a terminal environment, similar in spirit to a reliable AI butler.";
const SYSTEM_PROMPT_BEHAVIOR: &str = "Your job is to help the user get outcomes across many domains: system and environment checks, document and knowledge tasks, coding and debugging, shell workflows, planning, and everyday productivity. \
Be practical, accurate, and action-oriented. Prefer using available tools when verification or execution is needed. \
Keep responses concise, clear, and directly useful.";
const SYSTEM_PROMPT_SAFETY: &str =
    "For high-impact actions, surface assumptions and confirm intent before proceeding.";
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";
const STREAM_IDLE_TIMEOUT_SECS: u64 = 300;

fn default_system_prompt() -> String {
    format!(
        "{}\n\nBehavior guidelines:\n{}\n\nSafety boundaries:\n{}",
        SYSTEM_PROMPT_PERSONA, SYSTEM_PROMPT_BEHAVIOR, SYSTEM_PROMPT_SAFETY
    )
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

fn is_legacy_glm_chat_base_url(url: &str) -> bool {
    let normalized = url.trim_end_matches('/').to_ascii_lowercase();
    normalized.ends_with("/v4") || normalized.contains("/api/coding/paas/")
}

fn build_messages_endpoint(base_url: &str) -> String {
    let normalized = base_url.trim_end_matches('/');
    if normalized.ends_with("/v1") {
        format!("{normalized}/messages")
    } else {
        format!("{normalized}/v1/messages")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AnthropicMessagesClient {
    client: Client,
    model_id: String,
    provider: String,
    protocol: String,
    api_key: String,
    model: String,
    endpoint: String,
    anthropic_version: String,
    profile: AnthropicMessagesRequestProfile,
    backend_name: &'static str,
    thinking: Option<AnthropicThinking>,
    log_thinking: bool,
    capabilities: CapabilityFlags,
}

#[derive(Debug, Clone, Serialize)]
struct AnthropicThinking {
    #[serde(rename = "type")]
    thinking_type: String,
    budget_tokens: u32,
}

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinking>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

impl AnthropicMessagesClient {
    pub(crate) fn from_registered_model(entry: &RegisteredModel) -> Result<Option<Self>> {
        if entry.protocol != "anthropic_messages" {
            return Ok(None);
        }
        let profile = AnthropicMessagesRequestProfile::parse(entry.request_profile.as_deref())?;
        let api_key = entry
            .api_key
            .clone()
            .or_else(|| default_api_key_for_provider(&entry.provider))
            .ok_or_else(|| anyhow!("missing API key for provider '{}'", entry.provider))?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url_for_provider(&entry.provider).to_string());
        let anthropic_version = entry
            .anthropic_version
            .clone()
            .unwrap_or_else(|| DEFAULT_ANTHROPIC_VERSION.to_string());
        let thinking = entry
            .thinking_budget_tokens
            .map(|budget_tokens| AnthropicThinking {
                thinking_type: "enabled".to_string(),
                budget_tokens,
            });

        Ok(Some(Self {
            client: Client::new(),
            model_id: entry.id.clone(),
            provider: entry.provider.clone(),
            protocol: entry.protocol.clone(),
            api_key,
            model: entry.model.clone(),
            endpoint: build_messages_endpoint(&base_url),
            anthropic_version,
            profile,
            backend_name: "anthropic-messages",
            thinking,
            log_thinking: should_log_thinking(),
            capabilities: entry.capabilities,
        }))
    }

    pub(crate) fn from_anthropic_config(config: Option<&ModelConfig>) -> Option<Self> {
        Self::from_provider_config(
            config,
            "OPENJAX_ANTHROPIC_API_KEY",
            "OPENJAX_ANTHROPIC_MODEL",
            "OPENJAX_ANTHROPIC_BASE_URL",
            "claude-sonnet-4-5",
            "https://api.anthropic.com/v1",
            "anthropic-messages",
        )
    }

    pub(crate) fn from_glm_config(config: Option<&ModelConfig>) -> Option<Self> {
        Self::from_provider_config(
            config,
            "OPENJAX_GLM_API_KEY",
            "OPENJAX_GLM_MODEL",
            "OPENJAX_GLM_ANTHROPIC_BASE_URL",
            "GLM-4.7",
            "https://open.bigmodel.cn/api/anthropic",
            "glm-anthropic-messages",
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

        let env_base_url = std::env::var(base_url_env)
            .ok()
            .filter(|v| !v.trim().is_empty());
        let config_base_url = config
            .and_then(|c| c.base_url.clone())
            .filter(|v| !v.trim().is_empty());
        let base_url = if let Some(value) = env_base_url {
            value
        } else if backend_name == "glm-anthropic-messages" {
            match config_base_url {
                Some(value) if is_legacy_glm_chat_base_url(&value) => default_base_url.to_string(),
                Some(value) => value,
                None => default_base_url.to_string(),
            }
        } else {
            config_base_url.unwrap_or_else(|| default_base_url.to_string())
        };

        let anthropic_version = std::env::var("OPENJAX_ANTHROPIC_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ANTHROPIC_VERSION.to_string());

        let endpoint = build_messages_endpoint(&base_url);

        Some(Self {
            client: Client::new(),
            model_id: backend_name.to_string(),
            provider: backend_name
                .split('-')
                .next()
                .unwrap_or("anthropic")
                .to_string(),
            protocol: "anthropic_messages".to_string(),
            api_key,
            model,
            endpoint,
            anthropic_version,
            profile: AnthropicMessagesRequestProfile::Default,
            backend_name,
            thinking: load_thinking_from_env(),
            log_thinking: should_log_thinking(),
            capabilities: CapabilityFlags {
                stream: true,
                reasoning: true,
                tool_call: false,
                json_mode: false,
            },
        })
    }

    fn build_request(
        &self,
        request: &ModelRequest,
        stream: bool,
        thinking: Option<AnthropicThinking>,
    ) -> AnthropicMessagesRequest {
        let _profile = self.profile;
        AnthropicMessagesRequest {
            model: self.model.clone(),
            system: request
                .system_prompt
                .clone()
                .unwrap_or_else(default_system_prompt),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: request.user_input.to_string(),
            }],
            max_tokens: request.options.max_output_tokens.unwrap_or(4096),
            temperature: Some(0.2),
            stream: stream.then_some(true),
            thinking,
        }
    }

    fn request_builder(&self, accept: &str) -> reqwest::RequestBuilder {
        self.client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.anthropic_version)
            .header("accept", accept)
    }
}

fn default_api_key_for_provider(provider: &str) -> Option<String> {
    let key = match provider {
        "glm" => "OPENJAX_GLM_API_KEY",
        _ => "OPENJAX_ANTHROPIC_API_KEY",
    };
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn default_base_url_for_provider(provider: &str) -> &'static str {
    match provider {
        "glm" => "https://open.bigmodel.cn/api/anthropic",
        _ => "https://api.anthropic.com/v1",
    }
}

fn load_thinking_from_env() -> Option<AnthropicThinking> {
    let budget_tokens = std::env::var("OPENJAX_THINKING_BUDGET_TOKENS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)?;

    Some(AnthropicThinking {
        thinking_type: "enabled".to_string(),
        budget_tokens,
    })
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

fn summarize_log_preview(text: &str, limit: usize) -> (String, bool) {
    let normalized = text.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= limit {
        return (normalized, false);
    }

    let mut preview = normalized.chars().take(limit).collect::<String>();
    preview.push_str("...");
    (preview, true)
}

fn summarize_log_preview_json(text: &str, limit: usize) -> String {
    let (preview, truncated) = summarize_log_preview(text, limit);
    serde_json::json!({
        "thinking": preview,
        "truncated": truncated,
    })
    .to_string()
}

fn should_log_thinking() -> bool {
    !std::env::var("OPENJAX_LOG_THINKING")
        .is_ok_and(|v| matches!(v.as_str(), "0" | "false" | "FALSE"))
}

fn extract_content_from_body(body: &serde_json::Value) -> Option<String> {
    let content = body.get("content")?;

    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    if let Some(blocks) = content.as_array() {
        let mut merged = String::new();
        for block in blocks {
            if block.get("type").and_then(|v| v.as_str()) != Some("text") {
                continue;
            }
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

fn extract_thinking_from_body(body: &serde_json::Value) -> Option<String> {
    let blocks = body.get("content")?.as_array()?;
    let mut merged = String::new();
    for block in blocks {
        if block.get("type").and_then(|v| v.as_str()) != Some("thinking") {
            continue;
        }
        if let Some(thinking) = block.get("thinking").and_then(|v| v.as_str()) {
            if !merged.is_empty() {
                merged.push('\n');
            }
            merged.push_str(thinking);
        }
    }
    if merged.is_empty() {
        None
    } else {
        Some(merged)
    }
}

fn extract_delta_content_from_body(body: &serde_json::Value) -> Option<String> {
    if let Some(delta_text) = body
        .get("delta")
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        && !delta_text.is_empty()
    {
        return Some(delta_text.to_string());
    }

    if body.get("type").and_then(|v| v.as_str()) == Some("content_block_start")
        && let Some(text) = body
            .get("content_block")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
        && !text.is_empty()
    {
        return Some(text.to_string());
    }

    None
}

fn extract_delta_thinking_from_body(body: &serde_json::Value) -> Option<String> {
    if let Some(delta_thinking) = body
        .get("delta")
        .and_then(|v| v.get("thinking"))
        .and_then(|v| v.as_str())
        && !delta_thinking.is_empty()
    {
        return Some(delta_thinking.to_string());
    }

    if body.get("type").and_then(|v| v.as_str()) == Some("content_block_start")
        && body
            .get("content_block")
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str())
            == Some("thinking")
        && let Some(thinking) = body
            .get("content_block")
            .and_then(|v| v.get("thinking"))
            .and_then(|v| v.as_str())
        && !thinking.is_empty()
    {
        return Some(thinking.to_string());
    }

    None
}

#[async_trait]
impl ModelClient for AnthropicMessagesClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let thinking = request
            .options
            .thinking_budget_tokens
            .map(|budget_tokens| AnthropicThinking {
                thinking_type: "enabled".to_string(),
                budget_tokens,
            })
            .or_else(|| self.thinking.clone());
        let req = self.build_request(request, false, thinking);

        let resp = self
            .request_builder("application/json")
            .json(&req)
            .send()
            .await
            .context("anthropic messages request failed")?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .context("failed to read anthropic messages response body")?;

        let body_json: serde_json::Value = serde_json::from_str(&body_text).map_err(|err| {
            anyhow!(
                "failed to parse anthropic messages response JSON: {err}; status={status}; body_snippet={}",
                response_snippet(&body_text)
            )
        })?;

        if !status.is_success() {
            return Err(anyhow!(
                "anthropic messages API error ({status}): {}",
                response_snippet(&body_text)
            ));
        }

        if self.log_thinking
            && let Some(thinking_text) = extract_thinking_from_body(&body_json)
        {
            info!(
                backend = self.backend_name,
                endpoint = %self.endpoint,
                thinking_preview = %summarize_log_preview_json(&thinking_text, 600),
                "anthropic_thinking"
            );
        }

        let content = extract_content_from_body(&body_json).ok_or_else(|| {
            anyhow!(
                "missing content text in anthropic messages response; status={status}; body_snippet={}",
                response_snippet(&body_text)
            )
        })?;

        let usage = body_json.get("usage").map(|usage| ModelUsage {
            input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
            output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
            total_tokens: usage.get("total_tokens").and_then(|v| v.as_u64()),
        });

        let finish_reason = body_json
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        Ok(ModelResponse {
            text: content,
            reasoning: extract_thinking_from_body(&body_json),
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
        let thinking = request
            .options
            .thinking_budget_tokens
            .map(|budget_tokens| AnthropicThinking {
                thinking_type: "enabled".to_string(),
                budget_tokens,
            })
            .or_else(|| self.thinking.clone());
        let req = self.build_request(request, true, thinking);

        let resp = self
            .request_builder("text/event-stream")
            .json(&req)
            .send()
            .await
            .context("anthropic messages streaming request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .context("failed to read anthropic streaming error response body")?;
            return Err(anyhow!(
                "anthropic messages streaming API error ({status}): {}",
                response_snippet(&body_text)
            ));
        }

        let mut assembled = String::new();
        let mut thinking_assembled = String::new();
        let mut parser = AnthropicSseParser::default();
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
                    "anthropic messages stream timed out after {}s waiting for next chunk before [DONE]",
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

                if let Some(thinking_delta) = extract_delta_thinking_from_body(&payload)
                    && !thinking_delta.is_empty()
                {
                    thinking_assembled.push_str(&thinking_delta);
                    if let Some(sender) = &delta_sender {
                        let _ = sender.send(StreamDelta::Reasoning(thinking_delta));
                    }
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

            if let Some(thinking_delta) = extract_delta_thinking_from_body(&payload)
                && !thinking_delta.is_empty()
            {
                thinking_assembled.push_str(&thinking_delta);
                if let Some(sender) = &delta_sender {
                    let _ = sender.send(StreamDelta::Reasoning(thinking_delta));
                }
            }
        }

        if self.log_thinking && !thinking_assembled.is_empty() {
            info!(
                backend = self.backend_name,
                endpoint = %self.endpoint,
                thinking_preview = %summarize_log_preview_json(&thinking_assembled, 600),
                "anthropic_thinking_stream"
            );
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
                "anthropic messages stream ended before [DONE]; treating as protocol error"
            ));
        }

        if assembled.is_empty() {
            return Err(anyhow!(
                "missing streaming delta content in anthropic messages response; status={status}"
            ));
        }

        Ok(ModelResponse {
            text: assembled,
            reasoning: if thinking_assembled.is_empty() {
                None
            } else {
                Some(thinking_assembled)
            },
            usage: None,
            finish_reason: None,
            raw: None,
        })
    }

    fn name(&self) -> &'static str {
        self.backend_name
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicMessagesClient {
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
        AnthropicMessagesClient, build_messages_endpoint, extract_content_from_body,
        extract_delta_content_from_body, extract_delta_thinking_from_body,
        extract_thinking_from_body, is_legacy_glm_chat_base_url,
    };
    use crate::model::registry::RegisteredModel;
    use crate::model::types::{CapabilityFlags, ModelRequest, ModelStage};
    use crate::streaming::parser::parse_sse_data_line;

    #[test]
    fn extract_content_supports_text_blocks() {
        let body = json!({
            "content": [
                {"type": "thinking", "thinking": "internal"},
                {"type": "text", "text": "hello"},
                {"type": "text", "text": "world"}
            ]
        });

        let content = extract_content_from_body(&body);
        assert_eq!(content.as_deref(), Some("hello\nworld"));
    }

    #[test]
    fn extract_delta_supports_content_block_delta() {
        let body = json!({
            "type": "content_block_delta",
            "delta": {
                "type": "text_delta",
                "text": "hello"
            }
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
    fn extract_thinking_supports_thinking_blocks() {
        let body = json!({
            "content": [
                {"type": "thinking", "thinking": "step1"},
                {"type": "text", "text": "result"},
                {"type": "thinking", "thinking": "step2"}
            ]
        });

        let thinking = extract_thinking_from_body(&body);
        assert_eq!(thinking.as_deref(), Some("step1\nstep2"));
    }

    #[test]
    fn extract_delta_thinking_supports_content_block_delta() {
        let body = json!({
            "type": "content_block_delta",
            "delta": {
                "type": "thinking_delta",
                "thinking": "partial"
            }
        });

        let thinking = extract_delta_thinking_from_body(&body);
        assert_eq!(thinking.as_deref(), Some("partial"));
    }

    #[test]
    fn detect_legacy_glm_chat_base_url() {
        assert!(is_legacy_glm_chat_base_url(
            "https://open.bigmodel.cn/api/coding/paas/v4"
        ));
        assert!(is_legacy_glm_chat_base_url(
            "https://open.bigmodel.cn/api/coding/paas/v4/"
        ));
        assert!(!is_legacy_glm_chat_base_url(
            "https://open.bigmodel.cn/api/anthropic"
        ));
    }

    #[test]
    fn build_messages_endpoint_supports_base_with_or_without_v1() {
        assert_eq!(
            build_messages_endpoint("https://api.anthropic.com/v1"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            build_messages_endpoint("https://open.bigmodel.cn/api/anthropic"),
            "https://open.bigmodel.cn/api/anthropic/v1/messages"
        );
    }

    fn sample_registered_model(request_profile: Option<&str>) -> RegisteredModel {
        RegisteredModel {
            id: "anthropic".to_string(),
            provider: "anthropic".to_string(),
            protocol: "anthropic_messages".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            request_profile: request_profile.map(ToString::to_string),
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            api_key: Some("secret".to_string()),
            anthropic_version: Some("2023-06-01".to_string()),
            thinking_budget_tokens: None,
            capabilities: CapabilityFlags {
                stream: true,
                reasoning: true,
                tool_call: false,
                json_mode: false,
            },
        }
    }

    #[test]
    fn default_profile_keeps_anthropic_request_shape() {
        let client = AnthropicMessagesClient::from_registered_model(&sample_registered_model(None))
            .expect("build client")
            .expect("anthropic client");
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let req = client.build_request(&request, true, None);
        let http_request = client
            .request_builder("text/event-stream")
            .json(&req)
            .build()
            .expect("build request");

        assert_eq!(req.max_tokens, 4096);
        assert_eq!(req.temperature, Some(0.2));
        assert_eq!(req.stream, Some(true));
        assert_eq!(
            http_request
                .headers()
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some("2023-06-01")
        );
    }

    #[test]
    fn unknown_registered_model_profile_returns_clear_error() {
        let err = AnthropicMessagesClient::from_registered_model(&sample_registered_model(Some(
            "bad_profile",
        )))
        .expect_err("unknown profile should fail");
        assert!(
            err.to_string()
                .contains("unknown anthropic_messages request_profile")
        );
    }
}
