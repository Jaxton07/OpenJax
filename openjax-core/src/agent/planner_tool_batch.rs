use std::collections::HashSet;
use std::time::Instant;

use openjax_protocol::Event;
use tokio::task::JoinSet;
use tracing::info;

use crate::agent::decision::NormalizedToolCall;
use crate::agent::planner_utils::{
    extract_tool_target_hint, is_mutating_tool, tool_args_delta_payload, tool_failure_code,
    tool_failure_retryable,
};
use crate::agent::prompt::truncate_for_prompt;
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::tool_policy::is_approval_blocking_error;
use crate::agent::turn_engine::TurnEngine;
use crate::{Agent, tools};

impl Agent {
    pub(super) async fn execute_tool_batch_calls(
        &mut self,
        turn_id: u64,
        mut calls: Vec<NormalizedToolCall>,
        events: &mut Vec<Event>,
        tool_traces: &mut Vec<String>,
        apply_patch_read_guard: &mut ApplyPatchReadGuard,
        turn_engine: &mut TurnEngine,
    ) -> usize {
        let mut executed = 0usize;
        let mut succeeded = 0u32;
        let mut failed = 0u32;
        let total = calls.len() as u32;
        let mut completed_ids: HashSet<String> = HashSet::new();
        let batch_started_at = Instant::now();

        while !calls.is_empty() {
            let ready_indices = calls
                .iter()
                .enumerate()
                .filter_map(|(idx, call)| {
                    if call
                        .depends_on
                        .iter()
                        .all(|dep| completed_ids.contains(dep))
                    {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !ready_indices.is_empty() {
                let mut ready_calls = ready_indices
                    .into_iter()
                    .rev()
                    .map(|idx| calls.remove(idx))
                    .collect::<Vec<_>>();
                ready_calls.reverse();

                let mut join_set = JoinSet::new();
                for call in ready_calls {
                    if let Some(message) =
                        apply_patch_read_guard.block_user_message_for_tool(&call.tool_name)
                    {
                        self.push_event(
                            events,
                            Event::ToolCallStarted {
                                turn_id,
                                tool_call_id: call.tool_call_id.clone(),
                                tool_name: call.tool_name.clone(),
                                target: extract_tool_target_hint(&call.tool_name, &call.args),
                            },
                        );
                        if let Some(args_delta) = tool_args_delta_payload(&call.args) {
                            self.push_event(
                                events,
                                Event::ToolCallArgsDelta {
                                    turn_id,
                                    tool_call_id: call.tool_call_id.clone(),
                                    tool_name: call.tool_name.clone(),
                                    args_delta,
                                },
                            );
                        }
                        self.push_event(
                            events,
                            Event::ToolCallFailed {
                                turn_id,
                                tool_call_id: call.tool_call_id.clone(),
                                tool_name: call.tool_name.clone(),
                                code: "guard_blocked".to_string(),
                                message: message.to_string(),
                                retryable: false,
                            },
                        );
                        self.record_tool_call(&call.tool_name, &call.args, false, message);
                        tool_traces.push(format!(
                            "tool={}; ok=false; output={}",
                            call.tool_name,
                            truncate_for_prompt(
                                message,
                                self.skill_runtime_config.max_diff_chars_for_planner
                            )
                        ));
                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_call_id: call.tool_call_id.clone(),
                                tool_name: call.tool_name.clone(),
                                ok: false,
                                output: message.to_string(),
                            },
                        );
                        completed_ids.insert(call.tool_call_id);
                        executed += 1;
                        failed += 1;
                        continue;
                    }

                    self.push_event(
                        events,
                        Event::ToolCallStarted {
                            turn_id,
                            tool_call_id: call.tool_call_id.clone(),
                            tool_name: call.tool_name.clone(),
                            target: extract_tool_target_hint(&call.tool_name, &call.args),
                        },
                    );
                    if let Some(args_delta) = tool_args_delta_payload(&call.args) {
                        self.push_event(
                            events,
                            Event::ToolCallArgsDelta {
                                turn_id,
                                tool_call_id: call.tool_call_id.clone(),
                                tool_name: call.tool_name.clone(),
                                args_delta,
                            },
                        );
                    }
                    self.push_event(
                        events,
                        Event::ToolCallProgress {
                            turn_id,
                            tool_call_id: call.tool_call_id.clone(),
                            tool_name: call.tool_name.clone(),
                            progress_message: "scheduled".to_string(),
                        },
                    );
                    let tools = self.tools.clone();
                    let tool_runtime_config = self.tool_runtime_config;
                    let approval_handler = self.approval_handler.clone();
                    let cwd = self.cwd.clone();
                    join_set.spawn(async move {
                        let tool_call = tools::ToolCall {
                            name: call.tool_name.clone(),
                            args: call.args.clone(),
                        };
                        let result = tools
                            .execute(tools::ToolExecutionRequest {
                                turn_id,
                                tool_call_id: call.tool_call_id.clone(),
                                call: &tool_call,
                                cwd: cwd.as_path(),
                                config: tool_runtime_config,
                                approval_handler,
                                event_sink: None,
                            })
                            .await;
                        (call, result)
                    });
                }

                while let Some(result) = join_set.join_next().await {
                    match result {
                        Ok((call, Ok(outcome))) => {
                            let ok = outcome.success;
                            let output = outcome.output;
                            apply_patch_read_guard.on_tool_success(&call.tool_name);
                            if is_mutating_tool(&call.tool_name) {
                                self.state_epoch = self.state_epoch.saturating_add(1);
                            }
                            tool_traces.push(format!(
                                "tool={}; ok={}; output={}",
                                call.tool_name,
                                ok,
                                truncate_for_prompt(
                                    &output,
                                    self.skill_runtime_config.max_diff_chars_for_planner
                                )
                            ));
                            self.record_tool_call(&call.tool_name, &call.args, ok, &output);
                            self.push_event(
                                events,
                                Event::ToolCallCompleted {
                                    turn_id,
                                    tool_call_id: call.tool_call_id.clone(),
                                    tool_name: call.tool_name.clone(),
                                    ok,
                                    output,
                                },
                            );
                            completed_ids.insert(call.tool_call_id);
                            executed += 1;
                            if ok {
                                succeeded += 1;
                            } else {
                                failed += 1;
                            }
                        }
                        Ok((call, Err(err))) => {
                            let err_text = err.to_string();
                            let err_text_lower = err_text.to_ascii_lowercase();
                            apply_patch_read_guard.on_tool_failure(&call.tool_name, &err_text);
                            tool_traces.push(format!(
                                "tool={}; ok=false; output={}",
                                call.tool_name,
                                truncate_for_prompt(
                                    &err_text,
                                    self.skill_runtime_config.max_diff_chars_for_planner
                                )
                            ));
                            self.record_tool_call(&call.tool_name, &call.args, false, &err_text);
                            self.push_event(
                                events,
                                Event::ToolCallFailed {
                                    turn_id,
                                    tool_call_id: call.tool_call_id.clone(),
                                    tool_name: call.tool_name.clone(),
                                    code: tool_failure_code(&err_text).to_string(),
                                    message: err_text.clone(),
                                    retryable: tool_failure_retryable(&err_text),
                                },
                            );
                            self.push_event(
                                events,
                                Event::ToolCallCompleted {
                                    turn_id,
                                    tool_call_id: call.tool_call_id.clone(),
                                    tool_name: call.tool_name.clone(),
                                    ok: false,
                                    output: err_text.clone(),
                                },
                            );
                            if is_approval_blocking_error(&err_text) {
                                self.push_event(
                                    events,
                                    Event::ResponseError {
                                        turn_id,
                                        code: "approval_blocked".to_string(),
                                        message: "tool batch interrupted by approval decision"
                                            .to_string(),
                                        retryable: false,
                                    },
                                );
                                turn_engine.on_failed();
                            } else if err_text_lower.contains("timed out") {
                                self.push_event(
                                    events,
                                    Event::ResponseError {
                                        turn_id,
                                        code: "tool_timeout".to_string(),
                                        message: "tool execution timed out".to_string(),
                                        retryable: true,
                                    },
                                );
                            } else if err_text_lower.contains("cancel") {
                                self.push_event(
                                    events,
                                    Event::ResponseError {
                                        turn_id,
                                        code: "tool_canceled".to_string(),
                                        message: "tool execution canceled".to_string(),
                                        retryable: true,
                                    },
                                );
                            }
                            completed_ids.insert(call.tool_call_id);
                            executed += 1;
                            failed += 1;
                        }
                        Err(err) => {
                            let output = format!("tool task join failed: {err}");
                            self.push_event(
                                events,
                                Event::AssistantMessage {
                                    turn_id,
                                    content: output.clone(),
                                },
                            );
                            self.record_history("assistant", output);
                        }
                    }
                }
                continue;
            }

            let unresolved = calls.split_off(0);
            for call in unresolved {
                let output = "tool call dependency unmet".to_string();
                self.push_event(
                    events,
                    Event::ToolCallFailed {
                        turn_id,
                        tool_call_id: call.tool_call_id.clone(),
                        tool_name: call.tool_name.clone(),
                        code: "dependency_unmet".to_string(),
                        message: output.clone(),
                        retryable: false,
                    },
                );
                self.push_event(
                    events,
                    Event::ToolCallCompleted {
                        turn_id,
                        tool_call_id: call.tool_call_id.clone(),
                        tool_name: call.tool_name.clone(),
                        ok: false,
                        output: output.clone(),
                    },
                );
                tool_traces.push(format!(
                    "tool={}; ok=false; output={}",
                    call.tool_name,
                    truncate_for_prompt(
                        &output,
                        self.skill_runtime_config.max_diff_chars_for_planner
                    )
                ));
                self.record_tool_call(&call.tool_name, &call.args, false, &output);
                completed_ids.insert(call.tool_call_id);
                executed += 1;
                failed += 1;
            }
        }

        self.push_event(
            events,
            Event::ToolBatchCompleted {
                turn_id,
                total,
                succeeded,
                failed,
            },
        );
        info!(
            turn_id = turn_id,
            total = total,
            succeeded = succeeded,
            failed = failed,
            duration_ms = batch_started_at.elapsed().as_millis(),
            "tool_batch_completed"
        );
        executed
    }
}
