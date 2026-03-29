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
use crate::model::request_profiles::chat_completions::ChatCompletionsRequestProfile;
use crate::model::types::{
    AssistantContentBlock, CapabilityFlags, ConversationMessage, ModelRequest, ModelResponse,
    ModelUsage, StopReason, StreamDelta, UserContentBlock,
};
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
    profile: ChatCompletionsRequestProfile,
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
    messages: Vec<ChatApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ChatToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize)]
struct ChatApiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: ChatToolCallFunction,
}

#[derive(Debug, Serialize)]
struct ChatToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct ChatToolDef {
    #[serde(rename = "type")]
    kind: String,
    function: ChatToolFunction,
}

#[derive(Debug, Serialize)]
struct ChatToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

struct PendingToolCall {
    id: String,
    name: String,
    args: String,
}

impl ChatCompletionsClient {
    pub(crate) fn from_registered_model(entry: &RegisteredModel) -> Result<Option<Self>> {
        if entry.protocol != "chat_completions" {
            return Ok(None);
        }
        let profile = ChatCompletionsRequestProfile::parse(entry.request_profile.as_deref())?;
        let api_key = entry
            .api_key
            .clone()
            .or_else(|| default_api_key_for_provider(&entry.provider))
            .ok_or_else(|| anyhow!("missing API key for provider '{}'", entry.provider))?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url_for_provider(&entry.provider).to_string());
        Ok(Some(Self {
            client: Client::new(),
            model_id: entry.id.clone(),
            provider: entry.provider.clone(),
            protocol: entry.protocol.clone(),
            api_key,
            model: entry.model.clone(),
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
            profile,
            backend_name: "chat-completions",
            capabilities: entry.capabilities,
        }))
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
            profile: ChatCompletionsRequestProfile::Default,
            backend_name,
            capabilities: CapabilityFlags {
                stream: true,
                reasoning: false,
                tool_call: true,
                json_mode: false,
            },
        })
    }

    fn build_request(&self, request: &ModelRequest, stream: bool) -> ChatCompletionRequest {
        let system_content = request
            .system_prompt
            .clone()
            .unwrap_or_else(default_system_prompt);

        let mut messages = vec![ChatApiMessage {
            role: "system".to_string(),
            content: Some(system_content),
            tool_calls: None,
            tool_call_id: None,
        }];

        for msg in &request.messages {
            match msg {
                ConversationMessage::User(blocks) => {
                    let mut text_parts: Vec<&str> = Vec::new();
                    let mut tool_results: Vec<ChatApiMessage> = Vec::new();

                    for block in blocks {
                        match block {
                            UserContentBlock::Text { text } => {
                                text_parts.push(text.as_str());
                            }
                            UserContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                is_error: _,
                            } => {
                                tool_results.push(ChatApiMessage {
                                    role: "tool".to_string(),
                                    content: Some(content.clone()),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_use_id.clone()),
                                });
                            }
                        }
                    }

                    if !text_parts.is_empty() {
                        messages.push(ChatApiMessage {
                            role: "user".to_string(),
                            content: Some(text_parts.join("\n")),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    messages.extend(tool_results);
                }
                ConversationMessage::Assistant(blocks) => {
                    let text: String = blocks
                        .iter()
                        .filter_map(|b| {
                            match b {
                                AssistantContentBlock::Text { text } => Some(text.as_str()),
                                AssistantContentBlock::Reasoning { .. }
                                | AssistantContentBlock::ToolUse { .. } => None,
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    let tool_calls: Vec<ChatToolCall> = blocks
                        .iter()
                        .filter_map(|b| {
                            if let AssistantContentBlock::ToolUse { id, name, input } = b {
                                Some(ChatToolCall {
                                    id: id.clone(),
                                    kind: "function".to_string(),
                                    function: ChatToolCallFunction {
                                        name: name.clone(),
                                        arguments: serde_json::to_string(input).unwrap_or_default(),
                                    },
                                })
                            } else {
                                None
                            }
                        })
                        .collect();

                    messages.push(ChatApiMessage {
                        role: "assistant".to_string(),
                        content: if text.is_empty() { None } else { Some(text) },
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        tool_call_id: None,
                    });
                }
            }
        }

        let tools = request
            .tools
            .iter()
            .map(|spec| ChatToolDef {
                kind: "function".to_string(),
                function: ChatToolFunction {
                    name: spec.name.clone(),
                    description: spec.description.clone(),
                    parameters: spec.input_schema.clone(),
                },
            })
            .collect();

        ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            tools,
            temperature: None,
            max_tokens: self.profile.resolve_max_tokens(request).or(Some(32000)),
            stream: stream.then_some(true),
            stream_options: if stream && self.profile.include_stream_options() {
                Some(StreamOptions {
                    include_usage: true,
                })
            } else {
                None
            },
        }
    }

    fn request_builder(&self, accept: &str) -> reqwest::RequestBuilder {
        let builder = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .header("accept", accept)
            .header("User-Agent", concat!("openjax/", env!("CARGO_PKG_VERSION")));
        if let Some(user_agent) = self.profile.user_agent() {
            builder.header("User-Agent", user_agent)
        } else {
            builder
        }
    }
}

fn default_api_key_for_provider(_provider: &str) -> Option<String> {
    let key = "OPENAI_API_KEY";
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn default_base_url_for_provider(_provider: &str) -> &'static str {
    "https://api.openai.com/v1"
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

/// Extract text content and tool_calls from a non-streaming response body.
fn extract_content_blocks_from_body(body: &serde_json::Value) -> Vec<AssistantContentBlock> {
    let message = match body
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("message"))
    {
        Some(m) => m,
        None => return vec![],
    };

    let mut blocks = Vec::new();

    // Text content (may be null when only tool_calls are present)
    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            blocks.push(AssistantContentBlock::Text {
                text: text.to_string(),
            });
        }
    } else if let Some(content_blocks) = message.get("content").and_then(|v| v.as_array()) {
        let mut merged = String::new();
        for block in content_blocks {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                if !merged.is_empty() {
                    merged.push('\n');
                }
                merged.push_str(text);
            }
        }
        if !merged.is_empty() {
            blocks.push(AssistantContentBlock::Text { text: merged });
        }
    }

    // Tool calls
    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for tc in tool_calls {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args_str = func
                .and_then(|f| f.get("arguments"))
                .and_then(|v| v.as_str())
                .unwrap_or("{}");
            let input = serde_json::from_str(args_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            blocks.push(AssistantContentBlock::ToolUse { id, name, input });
        }
    }

    blocks
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

/// Extract streaming tool_call deltas from an SSE frame.
/// Returns a list of (index, Option<id>, Option<name>, Option<args_delta>).
fn extract_tool_call_deltas(
    body: &serde_json::Value,
) -> Vec<(usize, Option<String>, Option<String>, Option<String>)> {
    let tool_calls = match body
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("delta"))
        .and_then(|v| v.get("tool_calls"))
        .and_then(|v| v.as_array())
    {
        Some(arr) => arr,
        None => return vec![],
    };

    tool_calls
        .iter()
        .filter_map(|tc| {
            let index = tc.get("index").and_then(|v| v.as_u64())? as usize;
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let args = func
                .and_then(|f| f.get("arguments"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            Some((index, id, name, args))
        })
        .collect()
}

fn extract_finish_reason(body: &serde_json::Value) -> Option<String> {
    body.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("finish_reason"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

fn extract_usage_from_payload(payload: &serde_json::Value) -> Option<ModelUsage> {
    let usage_val = payload.get("usage")?;
    Some(ModelUsage {
        input_tokens: usage_val
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage_val.get("input_tokens").and_then(|v| v.as_u64())),
        output_tokens: usage_val
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage_val.get("output_tokens").and_then(|v| v.as_u64())),
        total_tokens: usage_val.get("total_tokens").and_then(|v| v.as_u64()),
    })
}

#[async_trait]
impl ModelClient for ChatCompletionsClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let req = self.build_request(request, false);

        let resp = self
            .request_builder("application/json")
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

        let content_blocks = extract_content_blocks_from_body(&body_json);
        if content_blocks.is_empty() {
            return Err(anyhow!(
                "missing choices[0].message content in API response; status={status}; body_snippet={}",
                response_snippet(&body_text)
            ));
        }

        let usage = extract_usage_from_payload(&body_json);

        let stop_reason = body_json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("finish_reason"))
            .and_then(|v| v.as_str())
            .map(StopReason::from_api_str);

        Ok(ModelResponse {
            content: content_blocks,
            reasoning: None,
            usage,
            stop_reason,
            raw: Some(body_json),
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let req = self.build_request(request, true);

        let resp = self
            .request_builder("text/event-stream")
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

        // Tool call streaming state
        let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut stream_stop_reason: Option<StopReason> = None;

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

                // Tool call delta processing
                for (index, id, name, args_delta) in extract_tool_call_deltas(&payload) {
                    // Grow pending_tool_calls if this is a new tool call
                    while pending_tool_calls.len() <= index {
                        let new_id = id.clone().unwrap_or_default();
                        let new_name = name.clone().unwrap_or_default();
                        if let Some(sender) = &delta_sender {
                            let _ = sender.send(StreamDelta::ToolUseStart {
                                id: new_id.clone(),
                                name: new_name.clone(),
                            });
                        }
                        pending_tool_calls.push(PendingToolCall {
                            id: new_id,
                            name: new_name,
                            args: String::new(),
                        });
                    }
                    if let Some(args) = args_delta
                        && !args.is_empty()
                    {
                        pending_tool_calls[index].args.push_str(&args);
                        if let Some(sender) = &delta_sender {
                            let _ = sender.send(StreamDelta::ToolArgsDelta {
                                id: pending_tool_calls[index].id.clone(),
                                delta: args,
                            });
                        }
                    }
                }

                if let Some(finish_reason) = extract_finish_reason(&payload) {
                    stream_stop_reason = Some(StopReason::from_api_str(&finish_reason));
                }

                if let Some(usage) = extract_usage_from_payload(&payload) {
                    last_usage = Some(usage);
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

            if let Some(usage) = extract_usage_from_payload(&payload) {
                last_usage = Some(usage);
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

        // Build final content blocks from assembled text + pending tool calls
        let mut content_blocks: Vec<AssistantContentBlock> = Vec::new();
        if !assembled.is_empty() {
            content_blocks.push(AssistantContentBlock::Text { text: assembled });
        }
        for tc in &pending_tool_calls {
            let input = serde_json::from_str(&tc.args)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            content_blocks.push(AssistantContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input,
            });
            if let Some(sender) = &delta_sender {
                let _ = sender.send(StreamDelta::ToolUseEnd { id: tc.id.clone() });
            }
        }

        if content_blocks.is_empty() {
            return Err(anyhow!(
                "missing streaming content in API response; status={status}"
            ));
        }

        Ok(ModelResponse {
            content: content_blocks,
            reasoning: None,
            usage: last_usage,
            stop_reason: stream_stop_reason,
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
        ChatCompletionsClient, extract_content_blocks_from_body, extract_delta_content_from_body,
        extract_delta_reasoning_from_body,
    };
    use crate::model::registry::RegisteredModel;
    use crate::model::types::{AssistantContentBlock, CapabilityFlags, ModelRequest, ModelStage};
    use crate::streaming::parser::parse_sse_data_line;

    #[test]
    fn extract_content_blocks_text_only() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": "hello world"
                    }
                }
            ]
        });

        let blocks = extract_content_blocks_from_body(&body);
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(&blocks[0], AssistantContentBlock::Text { text } if text == "hello world")
        );
    }

    #[test]
    fn extract_content_blocks_includes_tool_calls() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_abc",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"location\":\"Paris\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        });

        let blocks = extract_content_blocks_from_body(&body);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            AssistantContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "Paris");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn extract_content_blocks_text_and_tool_calls() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": "Let me check the weather.",
                        "tool_calls": [
                            {
                                "id": "call_xyz",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"location\":\"Tokyo\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        });

        let blocks = extract_content_blocks_from_body(&body);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], AssistantContentBlock::Text { .. }));
        assert!(matches!(&blocks[1], AssistantContentBlock::ToolUse { .. }));
    }

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

        let blocks = extract_content_blocks_from_body(&body);
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(&blocks[0], AssistantContentBlock::Text { text } if text == "hello\nworld")
        );
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

    fn sample_registered_model(request_profile: Option<&str>) -> RegisteredModel {
        RegisteredModel {
            id: "kimi".to_string(),
            provider: "kimi".to_string(),
            protocol: "chat_completions".to_string(),
            model: "kimi-for-coding".to_string(),
            request_profile: request_profile.map(ToString::to_string),
            base_url: Some("https://api.kimi.com/coding/v1".to_string()),
            api_key: Some("secret".to_string()),
            anthropic_version: None,
            thinking_budget_tokens: None,
            capabilities: CapabilityFlags {
                stream: true,
                reasoning: false,
                tool_call: false,
                json_mode: false,
            },
        }
    }

    #[test]
    fn kimi_profile_adds_max_tokens_and_disables_stream_options() {
        let client = ChatCompletionsClient::from_registered_model(&sample_registered_model(Some(
            "kimi_coding_v1",
        )))
        .expect("build client")
        .expect("chat client");
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let non_stream = client.build_request(&request, false);
        let stream = client.build_request(&request, true);

        assert_eq!(non_stream.max_tokens, Some(200_000));
        assert_eq!(stream.max_tokens, Some(200_000));
        assert!(stream.stream_options.is_none());
    }

    #[test]
    fn default_profile_keeps_stream_options_enabled() {
        let client = ChatCompletionsClient::from_registered_model(&sample_registered_model(None))
            .expect("build client")
            .expect("chat client");
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let stream = client.build_request(&request, true);

        assert_eq!(stream.max_tokens, Some(32000));
        assert!(stream.stream_options.is_some());
    }

    #[test]
    fn kimi_profile_sets_cli_user_agent_header() {
        let client = ChatCompletionsClient::from_registered_model(&sample_registered_model(Some(
            "kimi_coding_v1",
        )))
        .expect("build client")
        .expect("chat client");
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let http_request = client
            .request_builder("application/json")
            .json(&client.build_request(&request, false))
            .build()
            .expect("build request");

        assert_eq!(
            http_request
                .headers()
                .get("user-agent")
                .and_then(|value| value.to_str().ok()),
            Some(concat!("openjax/", env!("CARGO_PKG_VERSION")))
        );
    }

    #[test]
    fn unknown_registered_model_profile_returns_clear_error() {
        let err = ChatCompletionsClient::from_registered_model(&sample_registered_model(Some(
            "bad_profile",
        )))
        .expect_err("unknown profile should fail");
        assert!(
            err.to_string()
                .contains("unknown chat_completions request_profile")
        );
    }
}

#[cfg(test)]
mod streaming_usage_tests {
    use super::extract_usage_from_payload;

    #[test]
    fn test_usage_extracted_from_last_sse_frame() {
        let frame = r#"{"id":"test","choices":[{"index":0,"finish_reason":"stop","delta":{"content":""}}],"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}"#;
        let val: serde_json::Value = serde_json::from_str(frame).unwrap();
        let usage = extract_usage_from_payload(&val).unwrap();
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.total_tokens, Some(150));
    }

    #[test]
    fn test_usage_extracted_from_glm_format() {
        let frame = r#"{"usage":{"input_tokens":200,"output_tokens":80,"total_tokens":280}}"#;
        let val: serde_json::Value = serde_json::from_str(frame).unwrap();
        let usage = extract_usage_from_payload(&val).unwrap();
        assert_eq!(usage.input_tokens, Some(200));
        assert_eq!(usage.output_tokens, Some(80));
        assert_eq!(usage.total_tokens, Some(280));
    }
}
