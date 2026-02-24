use std::time::Instant;

use openjax_protocol::Event;
use tracing::{debug, info, warn};

use crate::agent::decision::{normalize_model_decision, parse_model_decision};
use crate::agent::prompt::{
    build_final_response_prompt, build_json_repair_prompt, build_planner_input, truncate_for_prompt,
};
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::tool_policy::{
    approval_rejected_stop_message, duplicate_skip_abort_message, duplicate_tool_call_warning,
    is_approval_rejected_error, should_abort_on_consecutive_duplicate_skips,
};
use crate::{
    Agent, MAX_CONSECUTIVE_DUPLICATE_SKIPS, MAX_PLANNER_ROUNDS_PER_TURN, MAX_TOOL_CALLS_PER_TURN,
    tools,
};

impl Agent {
    pub(crate) async fn execute_natural_language_turn(
        &mut self,
        turn_id: u64,
        user_input: &str,
        events: &mut Vec<Event>,
    ) {
        let mut tool_traces: Vec<String> = Vec::new();
        let mut executed_count = 0usize;
        let mut planner_rounds = 0usize;
        let mut consecutive_duplicate_skips = 0usize;
        let mut apply_patch_read_guard = ApplyPatchReadGuard::default();

        debug!(
            turn_id = turn_id,
            user_input_len = user_input.len(),
            "natural_language_turn started"
        );

        while executed_count < MAX_TOOL_CALLS_PER_TURN
            && planner_rounds < MAX_PLANNER_ROUNDS_PER_TURN
        {
            let remaining = MAX_TOOL_CALLS_PER_TURN - executed_count;
            let planner_input =
                build_planner_input(user_input, &self.history, &tool_traces, remaining);

            info!(
                turn_id = turn_id,
                phase = "thinking",
                planner_round = planner_rounds + 1,
                "llm_state"
            );

            self.apply_rate_limit().await;

            info!(
                turn_id = turn_id,
                executed_count = executed_count,
                remaining_calls = remaining,
                prompt_len = planner_input.len(),
                "model_request started"
            );
            planner_rounds += 1;

            let model_output = match self.model_client.complete(&planner_input).await {
                Ok(output) => output,
                Err(err) => {
                    warn!(
                        turn_id = turn_id,
                        phase = "failed",
                        error = %err,
                        "model_request failed"
                    );
                    let message = format!("[model error] {err}");
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
            };

            info!(
                turn_id = turn_id,
                output_len = model_output.len(),
                "model_request completed"
            );

            debug!(
                turn_id = turn_id,
                raw_output = %model_output,
                "model_raw_output"
            );

            let decision = if let Some(parsed) = parse_model_decision(&model_output) {
                normalize_model_decision(parsed)
            } else {
                info!(
                    turn_id = turn_id,
                    "model_output not valid JSON, attempting repair"
                );
                let repair_prompt = build_json_repair_prompt(&model_output);

                self.apply_rate_limit().await;

                info!(turn_id = turn_id, "model_repair_request started");

                let repaired_output = match self.model_client.complete(&repair_prompt).await {
                    Ok(output) => output,
                    Err(err) => {
                        warn!(
                            turn_id = turn_id,
                            error = %err,
                            "model_repair_request failed"
                        );
                        let message = format!("[model error] {err}");
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
                };

                info!(turn_id = turn_id, "model_repair_request completed");

                debug!(
                    turn_id = turn_id,
                    repaired_output = %repaired_output,
                    "model_repaired_output"
                );

                if let Some(parsed) = parse_model_decision(&repaired_output) {
                    normalize_model_decision(parsed)
                } else {
                    // Still non-JSON after one repair attempt: treat first output as final.
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: model_output.clone(),
                        },
                    );
                    self.record_history("assistant", model_output);
                    return;
                }
            };

            let action = decision.action.to_ascii_lowercase();

            info!(
                turn_id = turn_id,
                action = %action,
                tool = ?decision.tool,
                args = ?decision.args,
                message = ?decision.message,
                "model_decision"
            );

            if action == "final" {
                let seed_message = decision
                    .message
                    .unwrap_or_else(|| "任务已完成。".to_string());
                info!(
                    turn_id = turn_id,
                    phase = "completed",
                    action = "final",
                    "natural_language_turn completed"
                );
                let message = self
                    .stream_final_assistant_reply(
                        turn_id,
                        user_input,
                        &tool_traces,
                        &seed_message,
                        events,
                    )
                    .await;
                let (preview, preview_truncated) = summarize_log_preview(&message, 300);
                let preview_json = serde_json::json!({ "final_response": preview }).to_string();
                info!(
                    turn_id = turn_id,
                    output_len = message.len(),
                    output_preview = %preview_json,
                    output_truncated = preview_truncated,
                    "final_response"
                );
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

            if action == "tool" {
                let tool_name = match decision.tool {
                    Some(name) if !name.trim().is_empty() => name,
                    _ => {
                        warn!(turn_id = turn_id, "model_decision missing tool name");
                        let message = "[model error] tool action missing tool name".to_string();
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
                };

                let args = decision.args.clone().unwrap_or_default();

                if let Some(message) =
                    apply_patch_read_guard.block_user_message_for_tool(&tool_name)
                {
                    warn!(
                        turn_id = turn_id,
                        reason = apply_patch_read_guard
                            .block_log_reason_for_tool(&tool_name)
                            .unwrap_or("unknown"),
                        "apply_patch blocked by read-before-repatch guard"
                    );

                    self.push_event(
                        events,
                        Event::ToolCallStarted {
                            turn_id,
                            tool_name: tool_name.clone(),
                        },
                    );

                    self.record_tool_call(&tool_name, &args, false, &message);
                    tool_traces.push(format!(
                        "tool={tool_name}; ok=false; output={}",
                        truncate_for_prompt(&message)
                    ));
                    self.push_event(
                        events,
                        Event::ToolCallCompleted {
                            turn_id,
                            tool_name: tool_name.to_string(),
                            ok: false,
                            output: message.to_string(),
                        },
                    );
                    executed_count += 1;
                    consecutive_duplicate_skips = 0;
                    continue;
                }

                if self.is_duplicate_tool_call(&tool_name, &args) {
                    warn!(
                        turn_id = turn_id,
                        tool_name = %tool_name,
                        args = ?args,
                        "duplicate_tool_call detected, skipping"
                    );
                    let message = duplicate_tool_call_warning(&tool_name, &args);
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        },
                    );
                    self.record_history("assistant", message);
                    tool_traces.push(format!(
                        "tool={tool_name}; ok=skipped_duplicate; args={}",
                        serde_json::to_string(&args).unwrap_or_default()
                    ));
                    consecutive_duplicate_skips = consecutive_duplicate_skips.saturating_add(1);
                    if should_abort_on_consecutive_duplicate_skips(
                        consecutive_duplicate_skips,
                        MAX_CONSECUTIVE_DUPLICATE_SKIPS,
                    ) {
                        let loop_message =
                            duplicate_skip_abort_message(MAX_CONSECUTIVE_DUPLICATE_SKIPS);
                        self.push_event(
                            events,
                            Event::AssistantMessage {
                                turn_id,
                                content: loop_message.clone(),
                            },
                        );
                        self.record_history("assistant", loop_message);
                        return;
                    }
                    continue;
                }

                let call = tools::ToolCall {
                    name: tool_name.clone(),
                    args: args.clone(),
                };

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
                        tool_name: tool_name.clone(),
                    },
                );

                match self
                    .execute_tool_with_live_events(turn_id, &call, events)
                    .await
                {
                    Ok(output) => {
                        apply_patch_read_guard.on_tool_success(&tool_name);

                        if is_mutating_tool(&tool_name) {
                            // File/content state has changed. Move to a new epoch so read/list
                            // calls with the same args can run again against fresh state.
                            self.state_epoch = self.state_epoch.saturating_add(1);
                        }

                        let duration_ms = start_time.elapsed().as_millis();
                        info!(
                            turn_id = turn_id,
                            tool_name = %tool_name,
                            ok = true,
                            duration_ms = duration_ms,
                            output_len = output.len(),
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok=true; output={}",
                            truncate_for_prompt(&output)
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, true, &output);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                ok: true,
                                output: output.to_string(),
                            },
                        );
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                    }
                    Err(err) => {
                        let duration_ms = start_time.elapsed().as_millis();
                        let err_text = err.to_string();
                        apply_patch_read_guard.on_tool_failure(&tool_name, &err_text);
                        info!(
                            turn_id = turn_id,
                            tool_name = %tool_name,
                            ok = false,
                            duration_ms = duration_ms,
                            error = %err_text,
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok=false; output={}",
                            truncate_for_prompt(&err_text)
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, false, &err_text);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                ok: false,
                                output: err_text.to_string(),
                            },
                        );
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                        if is_approval_rejected_error(&err_text) {
                            let stop_message = approval_rejected_stop_message();
                            self.push_event(
                                events,
                                Event::AssistantMessage {
                                    turn_id,
                                    content: stop_message.clone(),
                                },
                            );
                            self.record_history("assistant", stop_message);
                            return;
                        }
                    }
                }

                continue;
            }

            warn!(
                turn_id = turn_id,
                action = %decision.action,
                "model_decision unsupported action"
            );
            let message = format!("[model error] unsupported action: {}", decision.action);
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

        let message = if executed_count >= MAX_TOOL_CALLS_PER_TURN {
            warn!(
                turn_id = turn_id,
                max_calls = MAX_TOOL_CALLS_PER_TURN,
                "natural_language_turn reached max tool calls"
            );
            format!(
                "已达到单回合最多 {} 次工具调用限制，请继续下一轮。",
                MAX_TOOL_CALLS_PER_TURN
            )
        } else {
            warn!(
                turn_id = turn_id,
                planner_rounds = planner_rounds,
                max_rounds = MAX_PLANNER_ROUNDS_PER_TURN,
                "natural_language_turn reached max planner rounds"
            );
            format!(
                "已达到单回合最多 {} 次规划轮次限制，请继续下一轮。",
                MAX_PLANNER_ROUNDS_PER_TURN
            )
        };
        self.push_event(
            events,
            Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            },
        );
        self.record_history("assistant", message);
    }

    pub(crate) async fn stream_final_assistant_reply(
        &mut self,
        turn_id: u64,
        user_input: &str,
        tool_traces: &[String],
        seed_message: &str,
        events: &mut Vec<Event>,
    ) -> String {
        let prompt = build_final_response_prompt(user_input, tool_traces, seed_message);
        self.apply_rate_limit().await;

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_future = self.model_client.complete_stream(&prompt, Some(delta_tx));
        tokio::pin!(stream_future);

        let mut streamed = String::new();
        let result = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta
                        && !delta.is_empty() {
                        streamed.push_str(&delta);
                        self.push_event(events, Event::AssistantDelta {
                            turn_id,
                            content_delta: delta,
                        });
                    }
                }
                result = &mut stream_future => {
                    break result;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            if !delta.is_empty() {
                streamed.push_str(&delta);
                self.push_event(
                    events,
                    Event::AssistantDelta {
                        turn_id,
                        content_delta: delta,
                    },
                );
            }
        }

        match result {
            Ok(full_text) => {
                if streamed.is_empty() {
                    return full_text;
                }
                if full_text.is_empty() {
                    return streamed;
                }
                if full_text == streamed {
                    return streamed;
                }
                full_text
            }
            Err(err) => {
                warn!(turn_id = turn_id, error = %err, "final response streaming failed; fallback to planner message");
                seed_message.to_string()
            }
        }
    }
}

fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "apply_patch" | "edit_file_range" | "shell" | "exec_command"
    )
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
