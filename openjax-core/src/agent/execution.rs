use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{Agent, tools};

#[derive(Debug, Clone)]
struct RetryConfig {
    max_retries: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 5000,
        }
    }
}

impl Agent {
    pub(crate) async fn execute_single_tool_call(
        &mut self,
        turn_id: u64,
        call: tools::ToolCall,
        events: &mut Vec<Event>,
    ) -> Option<(Vec<String>, String)> {
        let retry_config = RetryConfig::default();
        let start_time = Instant::now();
        let tool_call_id = Uuid::new_v4().to_string();

        info!(
            turn_id = turn_id,
            tool_call_id = %tool_call_id,
            tool_name = %call.name,
            args = ?call.args,
            "tool_call started"
        );

        self.emit_tool_call_started_sequence(
            turn_id,
            &tool_call_id,
            &call.name,
            &call.args,
            "executing",
            events,
        );

        // Try execution with retry
        let mut last_error = None;
        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                // Calculate delay with exponential backoff
                let delay = std::cmp::min(
                    retry_config.initial_delay_ms * 2u64.pow(attempt - 1),
                    retry_config.max_delay_ms,
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                warn!(
                    turn_id = turn_id,
                    tool_call_id = %tool_call_id,
                    tool_name = %call.name,
                    attempt = attempt,
                    "tool_call retry"
                );
            }

            match self
                .execute_tool_with_live_events(turn_id, &tool_call_id, &call, events)
                .await
            {
                Ok(outcome) => {
                    let output = outcome.output;
                    let ok = outcome.success;
                    let duration_ms = start_time.elapsed().as_millis();
                    info!(
                        turn_id = turn_id,
                        tool_call_id = %tool_call_id,
                        tool_name = %call.name,
                        ok = ok,
                        duration_ms = duration_ms,
                        output_len = output.len(),
                        "tool_call completed"
                    );
                    self.emit_tool_call_completed(
                        turn_id,
                        &tool_call_id,
                        &call.name,
                        ok,
                        &output,
                        events,
                    );
                    // Both ok=true and ok=false return Some — tool executed, result goes to history
                    let trace = format!(
                        "tool={}; ok={}; args={}; output={}",
                        call.name,
                        ok,
                        serde_json::to_string(&call.args).unwrap_or_default(),
                        crate::agent::prompt::truncate_for_prompt(&output, crate::MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT)
                    );
                    return Some((vec![trace], output));
                }
                Err(err) => {
                    last_error = Some(err);
                    // Check if error is retryable (not a validation error)
                    let err_str = last_error.as_ref().expect("last error set").to_string();
                    self.emit_tool_call_failed(
                        turn_id,
                        &tool_call_id,
                        &call.name,
                        &err_str,
                        events,
                    );
                    if err_str.contains("invalid")
                        || err_str.contains("permission denied")
                        || err_str.contains("Approval rejected")
                    {
                        // Non-retryable error, don't retry
                        break;
                    }
                }
            }
        }

        // All retries failed
        if let Some(err) = last_error {
            let duration_ms = start_time.elapsed().as_millis();
            info!(
                turn_id = turn_id,
                tool_call_id = %tool_call_id,
                tool_name = %call.name,
                ok = false,
                duration_ms = duration_ms,
                error = %err,
                "tool_call completed"
            );
            self.emit_tool_call_completed(
                turn_id,
                &tool_call_id,
                &call.name,
                false,
                &err.to_string(),
                events,
            );
            self.push_event(
                events,
                Event::ResponseError {
                    turn_id,
                    code: crate::agent::planner_utils::tool_failure_code(&err.to_string())
                        .to_string(),
                    message: err.to_string(),
                    retryable: crate::agent::planner_utils::tool_failure_retryable(
                        &err.to_string(),
                    ),
                },
            );
        }
        None
    }

    pub(crate) fn drain_tool_events(
        &self,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
        events: &mut Vec<Event>,
    ) {
        while let Ok(event) = rx.try_recv() {
            self.push_event(events, event);
        }
    }

    pub(crate) async fn execute_tool_with_live_events(
        &self,
        turn_id: u64,
        tool_call_id: &str,
        call: &tools::ToolCall,
        events: &mut Vec<Event>,
    ) -> anyhow::Result<tools::ToolExecOutcome> {
        let (tool_event_tx, mut tool_event_rx) = tokio::sync::mpsc::unbounded_channel();
        let execute_fut = self.tools.execute(tools::ToolExecutionRequest {
            turn_id,
            tool_call_id: tool_call_id.to_string(),
            call,
            cwd: self.cwd.as_path(),
            config: self.tool_runtime_config,
            approval_handler: self.approval_handler.clone(),
            event_sink: Some(tool_event_tx),
        });
        tokio::pin!(execute_fut);

        loop {
            tokio::select! {
                maybe_event = tool_event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.push_event(events, event);
                    }
                }
                result = &mut execute_fut => {
                    self.drain_tool_events(&mut tool_event_rx, events);
                    return result;
                }
            }
        }
    }
}
