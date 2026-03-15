use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};

use crate::agent::decision::{DecisionJsonStreamParser, parse_model_decision};
use crate::model::ModelRequest;
use crate::streaming::{
    ResponseStreamOrchestrator, emit_synthetic_response_deltas, run_stream_with_delta_handler,
};
use crate::{Agent, SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS};

#[derive(Debug, Default, Clone)]
pub(super) struct PlannerStreamResult {
    pub(super) model_output: String,
    pub(super) streamed_message: String,
    pub(super) live_streamed: bool,
    pub(super) action_hint: Option<String>,
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
        let mut stream_orchestrator =
            ResponseStreamOrchestrator::new(turn_id, openjax_protocol::StreamSource::ModelLive);

        let stream_result = run_stream_with_delta_handler(delta_rx, stream_future, |delta| {
            if delta.is_empty() {
                return;
            }
            let chunk = parser.push_chunk(&delta);
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
            for event in stream_orchestrator.on_delta(&chunk.message_delta) {
                self.push_event(events, event);
            }
        })
        .await;

        let mut fallback_reason: Option<&'static str> = None;
        let model_output = match stream_result {
            Ok(response) => {
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
            let fallback = self.model_client.complete(planner_request).await?;
            fallback.text
        };

        info!(
            turn_id = turn_id,
            planner_stream_total_ms = started_at.elapsed().as_millis(),
            live_streamed = response_started,
            delta_chars = streamed_message.chars().count(),
            "planner_stream_completed"
        );

        Ok(PlannerStreamResult {
            model_output: final_output,
            streamed_message,
            live_streamed: response_started,
            action_hint: parser.action().map(ToOwned::to_owned),
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
