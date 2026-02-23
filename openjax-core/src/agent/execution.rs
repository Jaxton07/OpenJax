use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};

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
    ) {
        let retry_config = RetryConfig::default();
        let start_time = Instant::now();

        info!(
            turn_id = turn_id,
            tool_name = %call.name,
            args = ?call.args,
            "tool_call started"
        );

        self.push_event(
            events,
            Event::ToolCallStarted {
                turn_id,
                tool_name: call.name.clone(),
            },
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
                self.push_event(
                    events,
                    Event::AssistantMessage {
                        turn_id,
                        content: format!("tool {} 第 {} 次重试...", call.name, attempt),
                    },
                );
                warn!(
                    turn_id = turn_id,
                    tool_name = %call.name,
                    attempt = attempt,
                    "tool_call retry"
                );
            }

            match self
                .execute_tool_with_live_events(turn_id, &call, events)
                .await
            {
                Ok(output) => {
                    let duration_ms = start_time.elapsed().as_millis();
                    info!(
                        turn_id = turn_id,
                        tool_name = %call.name,
                        ok = true,
                        duration_ms = duration_ms,
                        output_len = output.len(),
                        "tool_call completed"
                    );
                    if attempt > 0 {
                        self.push_event(
                            events,
                            Event::AssistantMessage {
                                turn_id,
                                content: format!("tool {} 重试成功", call.name),
                            },
                        );
                    }
                    self.push_event(
                        events,
                        Event::ToolCallCompleted {
                            turn_id,
                            tool_name: call.name.clone(),
                            ok: true,
                            output,
                        },
                    );
                    let message = format!("tool {} 执行成功", call.name);
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        },
                    );
                    self.record_history("assistant", message);
                    return;
                }
                Err(err) => {
                    last_error = Some(err);
                    // Check if error is retryable (not a validation error)
                    let err_str = last_error.as_ref().expect("last error set").to_string();
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
                tool_name = %call.name,
                ok = false,
                duration_ms = duration_ms,
                error = %err,
                "tool_call completed"
            );
            self.push_event(
                events,
                Event::ToolCallCompleted {
                    turn_id,
                    tool_name: call.name.clone(),
                    ok: false,
                    output: err.to_string(),
                },
            );
            let message = format!("tool {} 执行失败: {}", call.name, err);
            self.push_event(
                events,
                Event::AssistantMessage {
                    turn_id,
                    content: message.clone(),
                },
            );
            self.record_history("assistant", message);
        }
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
        call: &tools::ToolCall,
        events: &mut Vec<Event>,
    ) -> anyhow::Result<String> {
        let (tool_event_tx, mut tool_event_rx) = tokio::sync::mpsc::unbounded_channel();
        let execute_fut = self.tools.execute(
            turn_id,
            call,
            self.cwd.as_path(),
            self.tool_runtime_config,
            self.approval_handler.clone(),
            Some(tool_event_tx),
        );
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
