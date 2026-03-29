use std::collections::HashSet;
use std::time::Instant;

use openjax_protocol::Event;
use tracing::info;

use crate::agent::decision::NormalizedToolCall;
use crate::agent::planner_utils::is_mutating_tool;
use crate::agent::prompt::truncate_for_prompt;
use crate::agent::tool_policy::is_approval_blocking_error;
use crate::agent::turn_engine::TurnEngine;
use crate::{Agent, tools};

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct BatchExecutionResult {
    pub(super) executed_count: usize,
    pub(super) aborted_by_approval: bool,
    pub(super) error_emitted: bool,
}

impl Agent {
    #[allow(dead_code)]
    pub(super) async fn execute_tool_batch_calls(
        &mut self,
        turn_id: u64,
        mut calls: Vec<NormalizedToolCall>,
        events: &mut Vec<Event>,
        tool_traces: &mut Vec<String>,
        turn_engine: &mut TurnEngine,
    ) -> BatchExecutionResult {
        let mut executed = 0usize;
        let mut succeeded = 0u32;
        let mut failed = 0u32;
        let total = calls.len() as u32;
        let mut completed_ids: HashSet<String> = HashSet::new();
        let batch_started_at = Instant::now();
        let mut aborted_by_approval = false;
        let mut error_emitted = false;

        while !calls.is_empty() {
            if aborted_by_approval {
                break;
            }
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

                let mut tasks = Vec::new();
                for call in ready_calls {
                    self.emit_tool_call_started_sequence(
                        turn_id,
                        &call.tool_call_id,
                        &call.tool_name,
                        &call.args,
                        "scheduled",
                        events,
                    );
                    let tools = self.tools.clone();
                    let tool_runtime_config = self.tool_runtime_config;
                    let approval_handler = self.approval_handler.clone();
                    let cwd = self.cwd.clone();
                    let session_id = self.policy_session_id.clone();
                    let policy_runtime = self.policy_runtime.clone();
                    let call_for_task = call.clone();
                    let handle = tokio::spawn(async move {
                        let tool_call = tools::ToolCall {
                            name: call_for_task.tool_name.clone(),
                            args: call_for_task.args.clone(),
                        };
                        tools
                            .execute(tools::ToolExecutionRequest {
                                turn_id,
                                session_id,
                                tool_call_id: call_for_task.tool_call_id.clone(),
                                call: &tool_call,
                                cwd: cwd.as_path(),
                                config: tool_runtime_config,
                                approval_handler,
                                event_sink: None,
                                policy_runtime,
                            })
                            .await
                    });
                    tasks.push((call, handle));
                }

                let mut pending = tasks.into_iter().peekable();
                while let Some((call, handle)) = pending.next() {
                    match handle.await {
                        Ok(Ok(outcome)) => {
                            let ok = outcome.success;
                            let output = outcome.display_output;
                            if ok {
                                // no-op
                            } else {
                                // no-op
                            }
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
                            self.emit_tool_call_completed(
                                turn_id,
                                &call.tool_call_id,
                                &call.tool_name,
                                ok,
                                &output,
                                events,
                            );
                            completed_ids.insert(call.tool_call_id);
                            executed += 1;
                            if ok {
                                succeeded += 1;
                            } else {
                                failed += 1;
                            }
                        }
                        Ok(Err(err)) => {
                            let err_text = err.to_string();
                            let err_text_lower = err_text.to_ascii_lowercase();
                            tool_traces.push(format!(
                                "tool={}; ok=false; output={}",
                                call.tool_name,
                                truncate_for_prompt(
                                    &err_text,
                                    self.skill_runtime_config.max_diff_chars_for_planner
                                )
                            ));
                            self.record_tool_call(&call.tool_name, &call.args, false, &err_text);
                            self.emit_tool_call_failed(
                                turn_id,
                                &call.tool_call_id,
                                &call.tool_name,
                                &err_text,
                                events,
                            );
                            self.emit_tool_call_completed(
                                turn_id,
                                &call.tool_call_id,
                                &call.tool_name,
                                false,
                                &err_text,
                                events,
                            );
                            if is_approval_blocking_error(&err_text) {
                                if !error_emitted {
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
                                    error_emitted = true;
                                }
                                turn_engine.on_failed();
                                aborted_by_approval = true;
                                for (pending_call, pending_handle) in pending {
                                    pending_handle.abort();
                                    let canceled_output =
                                        "tool execution canceled by approval decision".to_string();
                                    self.emit_tool_call_failed(
                                        turn_id,
                                        &pending_call.tool_call_id,
                                        &pending_call.tool_name,
                                        &canceled_output,
                                        events,
                                    );
                                    self.emit_tool_call_completed(
                                        turn_id,
                                        &pending_call.tool_call_id,
                                        &pending_call.tool_name,
                                        false,
                                        &canceled_output,
                                        events,
                                    );
                                    self.record_tool_call(
                                        &pending_call.tool_name,
                                        &pending_call.args,
                                        false,
                                        &canceled_output,
                                    );
                                    tool_traces.push(format!(
                                        "tool={}; ok=false; output={}",
                                        pending_call.tool_name,
                                        truncate_for_prompt(
                                            &canceled_output,
                                            self.skill_runtime_config.max_diff_chars_for_planner
                                        )
                                    ));
                                    completed_ids.insert(pending_call.tool_call_id);
                                    executed += 1;
                                    failed += 1;
                                }
                                break;
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
                            self.emit_tool_call_failed(
                                turn_id,
                                &call.tool_call_id,
                                &call.tool_name,
                                &output,
                                events,
                            );
                            self.emit_tool_call_completed(
                                turn_id,
                                &call.tool_call_id,
                                &call.tool_name,
                                false,
                                &output,
                                events,
                            );
                            self.push_event(
                                events,
                                Event::ResponseError {
                                    turn_id,
                                    code: "tool_join_failed".to_string(),
                                    message: output.clone(),
                                    retryable: true,
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
                }
                if aborted_by_approval {
                    break;
                }
                continue;
            }
            if aborted_by_approval {
                break;
            }

            let unresolved = calls.split_off(0);
            for call in unresolved {
                let output = "tool call dependency unmet".to_string();
                self.emit_tool_call_started_sequence(
                    turn_id,
                    &call.tool_call_id,
                    &call.tool_name,
                    &call.args,
                    "dependency_unmet",
                    events,
                );
                self.push_event(
                    events,
                    Event::ToolCallFailed {
                        turn_id,
                        tool_call_id: call.tool_call_id.clone(),
                        tool_name: call.tool_name.clone(),
                        code: "dependency_unmet".to_string(),
                        message: output.clone(),
                        retryable: false,
                        display_name: self.tools.display_name_for(&call.tool_name),
                    },
                );
                self.emit_tool_call_completed(
                    turn_id,
                    &call.tool_call_id,
                    &call.tool_name,
                    false,
                    &output,
                    events,
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
        BatchExecutionResult {
            executed_count: executed,
            aborted_by_approval,
            error_emitted,
        }
    }
}
