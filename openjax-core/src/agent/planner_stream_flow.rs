use std::collections::HashMap;
use std::time::Instant;

use openjax_protocol::Event;
use tracing::info;

use crate::logger::AFTER_DISPATCH_LOG_TARGET;
use crate::model::{ModelRequest, ModelResponse, ModelUsage, StreamDelta};
use crate::streaming::{
    ResponseStreamOrchestrator, emit_synthetic_response_deltas, run_stream_with_delta_handler,
};
use crate::{Agent, SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS};

const AFTER_DISPATCH_PREFIX: &str = "OPENJAX_AFTER_DISPATCH";

fn reasoning_preview(input: &str, max_chars: usize) -> (String, bool) {
    let total = input.chars().count();
    let preview = input.chars().take(max_chars).collect::<String>();
    (preview, total > max_chars)
}

#[derive(Debug, Default, Clone)]
pub(super) struct PlannerStreamResult {
    pub(super) response: ModelResponse,
    pub(super) streamed_text: String,
    pub(super) live_streamed: bool,
    pub(super) usage: Option<ModelUsage>,
}

impl Agent {
    pub(super) async fn request_planner_model_output(
        &mut self,
        turn_id: u64,
        planner_request: &ModelRequest,
        emit_live_final_deltas: bool,
        events: &mut Vec<Event>,
    ) -> anyhow::Result<PlannerStreamResult> {
        let started_at = Instant::now();
        let (delta_tx, delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_future = self
            .model_client
            .complete_stream(planner_request, Some(delta_tx));

        let mut streamed_text = String::new();
        let mut response_started = false;
        let mut ttft_logged = false;
        let mut delta_event_count = 0u64;
        let mut last_live_delta_at: Option<Instant> = None;
        let mut stream_orchestrator =
            ResponseStreamOrchestrator::new(turn_id, openjax_protocol::StreamSource::ModelLive);
        let mut tool_names: HashMap<String, String> = HashMap::new();
        let mut args_accum: HashMap<String, String> = HashMap::new();

        let stream_result =
            run_stream_with_delta_handler(delta_rx, stream_future, |delta| match delta {
                StreamDelta::Text(text_delta) => {
                    if text_delta.is_empty() {
                        return;
                    }
                    if !ttft_logged {
                        ttft_logged = true;
                        info!(
                            turn_id = turn_id,
                            planner_stream_ttft_ms = started_at.elapsed().as_millis(),
                            "planner_stream_ttft"
                        );
                    }
                    streamed_text.push_str(&text_delta);
                    delta_event_count = delta_event_count.saturating_add(1);
                    last_live_delta_at = Some(Instant::now());
                    if emit_live_final_deltas {
                        response_started = true;
                        for event in stream_orchestrator.on_delta(&text_delta) {
                            self.push_event(events, event);
                        }
                    }
                }
                StreamDelta::ToolUseStart { id, name } => {
                    let display_name = self.tools.display_name_for(&name);
                    tool_names.insert(id.clone(), name.clone());
                    self.push_event(
                        events,
                        Event::ToolCallStarted {
                            turn_id,
                            tool_call_id: id,
                            tool_name: name,
                            target: None,
                            display_name,
                        },
                    );
                }
                StreamDelta::ToolArgsDelta { id, delta } => {
                    args_accum.entry(id.clone()).or_default().push_str(&delta);
                    let tool_name = tool_names.get(&id).cloned().unwrap_or_default();
                    let display_name = self.tools.display_name_for(&tool_name);
                    self.push_event(
                        events,
                        Event::ToolCallArgsDelta {
                            turn_id,
                            tool_call_id: id,
                            tool_name,
                            args_delta: delta,
                            display_name,
                        },
                    );
                }
                StreamDelta::ToolUseEnd { id } => {
                    let tool_name = tool_names.get(&id).cloned().unwrap_or_default();
                    let display_name = self.tools.display_name_for(&tool_name);
                    let target = args_accum
                        .remove(&id)
                        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
                        .and_then(|v| {
                            v.as_object().map(|obj| {
                                obj.iter()
                                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                    .collect::<HashMap<String, String>>()
                            })
                        })
                        .as_ref()
                        .and_then(|args| crate::agent::planner_utils::extract_tool_target_hint(&tool_name, args));
                    self.push_event(
                        events,
                        Event::ToolCallReady {
                            turn_id,
                            tool_call_id: id,
                            tool_name,
                            display_name,
                            target,
                        },
                    );
                }
                StreamDelta::Reasoning(reasoning_delta) => {
                    if reasoning_delta.is_empty() {
                        return;
                    }
                    let delta_len = reasoning_delta.chars().count();
                    let (preview, preview_truncated) = reasoning_preview(&reasoning_delta, 48);
                    info!(
                        target: AFTER_DISPATCH_LOG_TARGET,
                        turn_id = turn_id,
                        flow_prefix = AFTER_DISPATCH_PREFIX,
                        flow_node = "planner.reasoning.emit",
                        flow_route = "reasoning_delta",
                        flow_next = "gateway.reasoning_delta",
                        delta_len = delta_len,
                        delta_preview = %preview,
                        delta_preview_truncated = preview_truncated,
                        "after_dispatcher_trace"
                    );
                    self.push_event(
                        events,
                        Event::ReasoningDelta {
                            turn_id,
                            content_delta: reasoning_delta,
                            stream_source: openjax_protocol::StreamSource::ModelLive,
                        },
                    );
                }
            })
            .await;

        let response = stream_result?;
        if emit_live_final_deltas
            && !response.has_tool_use()
            && !streamed_text.is_empty()
            && !response_started
        {
            response_started = true;
            for event in stream_orchestrator.on_delta(&streamed_text) {
                self.push_event(events, event);
            }
        }
        let stream_phase_total_ms = started_at.elapsed().as_millis() as u64;
        let captured_usage = response.usage.clone();

        info!(
            turn_id = turn_id,
            planner_stream_total_ms = started_at.elapsed().as_millis(),
            live_streamed = response_started,
            delta_events = delta_event_count,
            tail_silence_ms = last_live_delta_at
                .map(|ts| ts.elapsed().as_millis() as u64)
                .unwrap_or(stream_phase_total_ms),
            delta_chars = streamed_text.chars().count(),
            "planner_stream_completed"
        );

        Ok(PlannerStreamResult {
            response,
            streamed_text,
            live_streamed: response_started,
            usage: captured_usage,
        })
    }

    pub(super) fn emit_synthetic_response_deltas(
        &mut self,
        turn_id: u64,
        message: &str,
        events: &mut Vec<Event>,
    ) {
        for event in
            emit_synthetic_response_deltas(turn_id, message, SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS)
        {
            self.push_event(events, event);
        }
    }
}
