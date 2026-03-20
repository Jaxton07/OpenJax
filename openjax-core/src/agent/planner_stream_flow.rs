use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};

use crate::agent::decision::{DecisionJsonStreamParser, parse_model_decision};
use crate::logger::AFTER_DISPATCH_LOG_TARGET;
use crate::model::{ModelRequest, ModelUsage, StreamDelta};
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
    pub(super) model_output: String,
    pub(super) streamed_message: String,
    pub(super) live_streamed: bool,
    pub(super) action_hint: Option<String>,
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

        let mut parser = DecisionJsonStreamParser::new();
        let mut streamed_message = String::new();
        let mut response_started = false;
        let mut ttft_logged = false;
        let mut delta_event_count = 0u64;
        let mut last_live_delta_at: Option<Instant> = None;
        let mut stream_orchestrator =
            ResponseStreamOrchestrator::new(turn_id, openjax_protocol::StreamSource::ModelLive);

        let stream_result =
            run_stream_with_delta_handler(delta_rx, stream_future, |delta| match delta {
                StreamDelta::Text(text_delta) => {
                    if text_delta.is_empty() {
                        return;
                    }
                    let chunk = parser.push_chunk(&text_delta);
                    if !emit_live_final_deltas || chunk.message_delta.is_empty() {
                        return;
                    }
                    if !response_started {
                        response_started = true;
                    }
                    if !ttft_logged {
                        ttft_logged = true;
                        info!(
                            turn_id = turn_id,
                            planner_stream_ttft_ms = started_at.elapsed().as_millis(),
                            "planner_stream_ttft"
                        );
                    }
                    streamed_message.push_str(&chunk.message_delta);
                    delta_event_count = delta_event_count.saturating_add(1);
                    last_live_delta_at = Some(Instant::now());
                    for event in stream_orchestrator.on_delta(&chunk.message_delta) {
                        self.push_event(events, event);
                    }
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

        let mut captured_usage: Option<ModelUsage> = None;
        let mut fallback_reason: Option<&'static str> = None;
        let model_output = match stream_result {
            Ok(response) => {
                captured_usage = response.usage;
                let output = response.text;
                if parse_model_decision(&output).is_some() {
                    output
                } else if emit_live_final_deltas
                    && parser.action() == Some("final")
                    && !streamed_message.is_empty()
                {
                    format!(
                        "{{\"action\":\"final\",\"message\":{}}}",
                        serde_json::to_string(&streamed_message)
                            .unwrap_or_else(|_| "\"\"".to_string())
                    )
                } else {
                    fallback_reason = Some("parse_failed_after_stream");
                    parser.raw_text().to_string()
                }
            }
            Err(err) => {
                warn!(
                    turn_id = turn_id,
                    error = %err,
                    "planner_stream_failed"
                );
                fallback_reason = Some("stream_failed");
                parser.raw_text().to_string()
            }
        };
        let stream_phase_total_ms = started_at.elapsed().as_millis() as u64;

        let mut fallback_complete_ms: Option<u64> = None;
        let final_output = if parse_model_decision(&model_output).is_some() {
            model_output
        } else {
            if matches!(
                fallback_reason,
                Some("parse_failed_after_stream") | Some("parse_failed")
            ) {
                info!(
                    turn_id = turn_id,
                    planner_stream_parse_error_count = 1,
                    "planner_stream_metric"
                );
            }
            info!(
                turn_id = turn_id,
                fallback_reason = fallback_reason.unwrap_or("parse_failed"),
                "planner_stream_fallback_to_complete"
            );
            info!(
                turn_id = turn_id,
                planner_stream_fallback_count = 1,
                "planner_stream_metric"
            );
            let fallback_started_at = Instant::now();
            let fallback = self.model_client.complete(planner_request).await?;
            fallback_complete_ms = Some(fallback_started_at.elapsed().as_millis() as u64);
            captured_usage = fallback.usage;
            fallback.text
        };

        info!(
            turn_id = turn_id,
            planner_stream_total_ms = started_at.elapsed().as_millis(),
            live_streamed = response_started,
            delta_events = delta_event_count,
            tail_silence_ms = last_live_delta_at
                .map(|ts| ts.elapsed().as_millis() as u64)
                .unwrap_or(stream_phase_total_ms),
            fallback_complete_ms = fallback_complete_ms,
            delta_chars = streamed_message.chars().count(),
            "planner_stream_completed"
        );

        Ok(PlannerStreamResult {
            model_output: final_output,
            streamed_message,
            live_streamed: response_started,
            action_hint: parser.action().map(ToOwned::to_owned),
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
