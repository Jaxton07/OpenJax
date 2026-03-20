use std::collections::BTreeMap;

use openjax_protocol::Event;
use tracing::{debug, info, warn};

use crate::Agent;
use crate::agent::planner_tool_batch::BatchExecutionResult;
use crate::agent::planner_utils::{summarize_log_preview, summarize_log_preview_json};
use crate::agent::prompt::build_planner_input;
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::turn_engine::TurnEngine;
use crate::dispatcher::{self, DispatchOutcome, ProbeInput};
use crate::logger::AFTER_DISPATCH_LOG_TARGET;
use crate::model::{ModelRequest, ModelStage};

const FLOW_TRACE_PREFIX: &str = "OPENJAX_FLOW";
const AFTER_DISPATCH_PREFIX: &str = "OPENJAX_AFTER_DISPATCH";

/// Tool action context to reduce parameter count in handle_tool_action
pub(crate) struct ToolActionContext<'a> {
    pub events: &'a mut Vec<Event>,
    pub tool_traces: &'a mut Vec<String>,
    pub apply_patch_read_guard: &'a mut ApplyPatchReadGuard,
    pub consecutive_duplicate_skips: &'a mut usize,
    pub executed_count: &'a mut usize,
    pub turn_engine: &'a mut TurnEngine,
    pub skill_shell_misfire_count: &'a mut usize,
    pub saw_git_status_short: &'a mut bool,
    pub saw_git_diff_stat: &'a mut bool,
    pub diff_strategy: &'a mut &'static str,
}

fn log_after_dispatch_step(
    turn_id: u64,
    node: &'static str,
    route: &'static str,
    next: &'static str,
    result: Option<&'static str>,
    code: Option<&str>,
) {
    tracing::info!(
        target: AFTER_DISPATCH_LOG_TARGET,
        turn_id = turn_id,
        flow_prefix = AFTER_DISPATCH_PREFIX,
        flow_node = node,
        flow_route = route,
        flow_next = next,
        flow_result = result,
        flow_code = code,
        "after_dispatcher_trace"
    );
}

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
        let mut skill_shell_misfire_count = 0usize;
        let mut saw_git_status_short = false;
        let mut saw_git_diff_stat = false;
        let mut diff_strategy: &'static str = "none";
        let mut turn_engine = TurnEngine::new();

        debug!(
            turn_id = turn_id,
            user_input_len = user_input.len(),
            "natural_language_turn started"
        );

        self.loop_detector.reset();

        while executed_count < self.max_tool_calls_per_turn
            && planner_rounds < self.max_planner_rounds_per_turn
        {
            let remaining = self.max_tool_calls_per_turn - executed_count;
            let selected_skills = if self.skill_runtime_config.enabled {
                self.skill_registry
                    .select_for_input(user_input, self.skill_runtime_config.max_selected)
            } else {
                Vec::new()
            };
            let skills_context = if self.skill_runtime_config.enabled {
                crate::skills::build_skills_context(
                    &selected_skills,
                    self.skill_runtime_config.max_prompt_chars,
                )
            } else {
                "(skills disabled)".to_string()
            };
            let loop_recovery = self.loop_detector.recovery_prompt();
            let planner_input = build_planner_input(
                user_input,
                &self.history,
                &tool_traces,
                remaining,
                &skills_context,
                loop_recovery,
            );

            info!(
                turn_id = turn_id,
                skills_selected = selected_skills.len(),
                skills_prompt_chars = skills_context.chars().count(),
                "skills_context prepared"
            );

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

            let planner_request = ModelRequest::for_stage(ModelStage::Planner, planner_input);
            let planner_stream = match self
                .request_planner_model_output(turn_id, &planner_request, true, events)
                .await
            {
                Ok(streamed) => streamed,
                Err(err) => {
                    warn!(
                        turn_id = turn_id,
                        phase = "failed",
                        error = %err,
                        "model_request failed"
                    );
                    self.push_event(
                        events,
                        Event::ResponseError {
                            turn_id,
                            code: "model_request_failed".to_string(),
                            message: format!("[model error] {err}"),
                            retryable: true,
                        },
                    );
                    turn_engine.on_failed();
                    return;
                }
            };
            let model_output = planner_stream.model_output;
            let mut routed = dispatcher::route_model_output(
                ProbeInput {
                    turn_id,
                    action_hint: planner_stream.action_hint.as_deref(),
                    model_output: Some(&model_output),
                },
                &model_output,
                self.tool_batch_v2_enabled,
                self.dispatcher_config,
            );
            let mut used_repair = false;

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

            if let DispatchOutcome::Repair { raw_output, .. } = &routed {
                info!(
                    turn_id = turn_id,
                    "model_output not valid JSON, attempting repair"
                );
                let repair_prompt = crate::agent::prompt::build_json_repair_prompt(raw_output);

                self.apply_rate_limit().await;

                info!(turn_id = turn_id, "model_repair_request started");

                let repair_request = ModelRequest::for_stage(ModelStage::Planner, repair_prompt);
                let repaired_output = match self.model_client.complete(&repair_request).await {
                    Ok(output) => output.text,
                    Err(err) => {
                        warn!(
                            turn_id = turn_id,
                            error = %err,
                            "model_repair_request failed"
                        );
                        self.push_event(
                            events,
                            Event::ResponseError {
                                turn_id,
                                code: "model_repair_failed".to_string(),
                                message: format!("[model error] {err}"),
                                retryable: true,
                            },
                        );
                        turn_engine.on_failed();
                        return;
                    }
                };

                info!(
                    turn_id = turn_id,
                    output_len = repaired_output.len(),
                    output_preview = %summarize_log_preview_json(&repaired_output, 400),
                    output_truncated = repaired_output.chars().count() > 400,
                    "model_repair_request completed"
                );

                debug!(
                    turn_id = turn_id,
                    repaired_output = %repaired_output,
                    "model_repaired_output"
                );

                routed = dispatcher::route_model_output(
                    ProbeInput {
                        turn_id,
                        action_hint: planner_stream.action_hint.as_deref(),
                        model_output: Some(&repaired_output),
                    },
                    &repaired_output,
                    self.tool_batch_v2_enabled,
                    self.dispatcher_config,
                );
                used_repair = true;
            }

            match routed {
                DispatchOutcome::ToolBatch { meta, mut calls } => {
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.dispatch_consume",
                        flow_route = "tool_batch",
                        flow_next = "planner.tool_batch.execute",
                        tool_calls = calls.len(),
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "flow_trace"
                    );
                    info!(
                        turn_id = turn_id,
                        action = "tool_batch",
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "model_decision"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.dispatch_consume",
                        "tool_batch",
                        "planner.tool_batch.execute",
                        None,
                        None,
                    );
                    if calls.len() > remaining {
                        calls.truncate(remaining);
                    }
                    let proposals = calls
                        .iter()
                        .map(|call| openjax_protocol::ToolCallProposal {
                            tool_call_id: call.tool_call_id.clone(),
                            tool_name: call.tool_name.clone(),
                            arguments: call
                                .args
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect::<BTreeMap<_, _>>(),
                            depends_on: call.depends_on.clone(),
                            concurrency_group: call.concurrency_group.clone(),
                        })
                        .collect::<Vec<_>>();
                    self.push_event(
                        events,
                        Event::ToolCallsProposed {
                            turn_id,
                            tool_calls: proposals,
                        },
                    );
                    turn_engine.on_tool_batch_started();
                    let batch_result: BatchExecutionResult = self
                        .execute_tool_batch_calls(
                            turn_id,
                            calls,
                            events,
                            &mut tool_traces,
                            &mut apply_patch_read_guard,
                            &mut turn_engine,
                        )
                        .await;
                    executed_count += batch_result.executed_count;
                    consecutive_duplicate_skips = 0;
                    if batch_result.aborted_by_approval {
                        tracing::info!(
                            turn_id = turn_id,
                            flow_prefix = FLOW_TRACE_PREFIX,
                            flow_node = "planner.tool_batch.result",
                            flow_result = "aborted_by_approval",
                            flow_next = "turn.failed",
                            executed_count = batch_result.executed_count,
                            "flow_trace"
                        );
                        if !batch_result.error_emitted {
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
                        }
                        log_after_dispatch_step(
                            turn_id,
                            "planner.tool_batch.result",
                            "tool_batch",
                            "turn.failed",
                            Some("aborted_by_approval"),
                            Some("approval_blocked"),
                        );
                        turn_engine.on_failed();
                        return;
                    }
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.tool_batch.result",
                        flow_result = "completed",
                        flow_next = "planner.next_round",
                        executed_count = batch_result.executed_count,
                        "flow_trace"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.tool_batch.result",
                        "tool_batch",
                        "planner.next_round",
                        Some("completed"),
                        None,
                    );
                    turn_engine.on_response_resumed();
                    continue;
                }
                DispatchOutcome::Final { meta, decision } => {
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.dispatch_consume",
                        flow_route = "final",
                        flow_next = "frontend.response_stream",
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "flow_trace"
                    );
                    info!(
                        turn_id = turn_id,
                        action = "final",
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "model_decision"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.dispatch_consume",
                        "final",
                        "frontend.response_stream",
                        None,
                        None,
                    );
                    debug!(
                        turn_id = turn_id,
                        message = ?decision.message,
                        "model_decision_payload"
                    );
                    let seed_message = decision
                        .message
                        .unwrap_or_else(|| "任务已完成。".to_string());
                    info!(
                        turn_id = turn_id,
                        phase = "completed",
                        action = "final",
                        final_response_mode = "planner_only",
                        skill_shell_misfire_count = skill_shell_misfire_count,
                        skill_workflow_shortcut_used = saw_git_status_short && saw_git_diff_stat,
                        diff_strategy = diff_strategy,
                        "natural_language_turn completed"
                    );
                    let used_repair_with_live_stream = planner_stream.live_streamed && used_repair;
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.final.emit",
                        flow_route = "final",
                        flow_next = "frontend.response_completed",
                        live_streamed = planner_stream.live_streamed,
                        used_repair = used_repair,
                        used_repair_with_live_stream = used_repair_with_live_stream,
                        "flow_trace"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.final.emit",
                        "final",
                        "frontend.response_completed",
                        Some("completed"),
                        None,
                    );
                    let message = if planner_stream.live_streamed {
                        turn_engine.on_response_started();
                        let completed_content = if planner_stream.streamed_message.is_empty() {
                            seed_message.clone()
                        } else {
                            planner_stream.streamed_message.clone()
                        };
                        self.push_event(
                            events,
                            Event::ResponseCompleted {
                                turn_id,
                                content: completed_content.clone(),
                                stream_source: openjax_protocol::StreamSource::ModelLive,
                            },
                        );
                        completed_content
                    } else {
                        turn_engine.on_response_started();
                        self.push_event(
                            events,
                            Event::ResponseStarted {
                                turn_id,
                                stream_source: openjax_protocol::StreamSource::Synthetic,
                            },
                        );
                        self.emit_synthetic_response_deltas(turn_id, &seed_message, events);
                        self.push_event(
                            events,
                            Event::ResponseCompleted {
                                turn_id,
                                content: seed_message.clone(),
                                stream_source: openjax_protocol::StreamSource::Synthetic,
                            },
                        );
                        seed_message
                    };
                    let (preview, preview_truncated) = summarize_log_preview(&message, 300);
                    let preview_json = serde_json::json!({ "final_response": preview }).to_string();
                    info!(
                        turn_id = turn_id,
                        output_len = message.len(),
                        output_preview = %preview_json,
                        output_truncated = preview_truncated,
                        used_repair_with_live_stream = used_repair_with_live_stream,
                        "final_response"
                    );
                    turn_engine.on_completed();
                    self.commit_turn(user_input.to_string(), tool_traces, message);
                    return;
                }
                DispatchOutcome::Tool { meta, decision } => {
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.dispatch_consume",
                        flow_route = "tool",
                        flow_next = "planner.tool_action.execute",
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "flow_trace"
                    );
                    info!(
                        turn_id = turn_id,
                        action = "tool",
                        tool = ?decision.tool,
                        conflict_detected = meta.conflict_detected,
                        signal_source = meta.signal_source.as_str(),
                        "model_decision"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.dispatch_consume",
                        "tool",
                        "planner.tool_action.execute",
                        None,
                        None,
                    );
                    debug!(
                        turn_id = turn_id,
                        args = ?decision.args,
                        "model_decision_payload"
                    );
                    let mut ctx = ToolActionContext {
                        events,
                        tool_traces: &mut tool_traces,
                        apply_patch_read_guard: &mut apply_patch_read_guard,
                        consecutive_duplicate_skips: &mut consecutive_duplicate_skips,
                        executed_count: &mut executed_count,
                        turn_engine: &mut turn_engine,
                        skill_shell_misfire_count: &mut skill_shell_misfire_count,
                        saw_git_status_short: &mut saw_git_status_short,
                        saw_git_diff_stat: &mut saw_git_diff_stat,
                        diff_strategy: &mut diff_strategy,
                    };
                    let should_continue =
                        self.handle_tool_action(turn_id, &decision, &mut ctx).await;
                    if should_continue {
                        continue;
                    }
                    return;
                }
                DispatchOutcome::Repair { meta, reason, .. } => {
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.repair.result",
                        flow_route = "repair",
                        flow_result = "exhausted",
                        flow_next = "turn.failed",
                        reason = reason,
                        conflict_detected = meta.conflict_detected,
                        "flow_trace"
                    );
                    warn!(
                        turn_id = turn_id,
                        reason = reason,
                        conflict_detected = meta.conflict_detected,
                        "model_decision_repair_exhausted"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.repair.result",
                        "repair",
                        "turn.failed",
                        Some("exhausted"),
                        Some("model_decision_parse_failed"),
                    );
                    self.push_event(
                        events,
                        Event::ResponseError {
                            turn_id,
                            code: "model_decision_parse_failed".to_string(),
                            message: "model output parse failed after repair".to_string(),
                            retryable: true,
                        },
                    );
                    turn_engine.on_failed();
                    return;
                }
                DispatchOutcome::Error { code, message } => {
                    tracing::info!(
                        turn_id = turn_id,
                        flow_prefix = FLOW_TRACE_PREFIX,
                        flow_node = "planner.dispatch_consume",
                        flow_route = "error",
                        flow_next = "turn.failed",
                        flow_code = code,
                        "flow_trace"
                    );
                    log_after_dispatch_step(
                        turn_id,
                        "planner.dispatch_consume",
                        "error",
                        "turn.failed",
                        Some("failed"),
                        Some(code),
                    );
                    self.push_event(
                        events,
                        Event::ResponseError {
                            turn_id,
                            code: code.to_string(),
                            message,
                            retryable: false,
                        },
                    );
                    turn_engine.on_failed();
                    return;
                }
            }
        }

        let message = if executed_count >= self.max_tool_calls_per_turn {
            warn!(
                turn_id = turn_id,
                max_calls = self.max_tool_calls_per_turn,
                skill_shell_misfire_count = skill_shell_misfire_count,
                skill_workflow_shortcut_used = saw_git_status_short && saw_git_diff_stat,
                diff_strategy = diff_strategy,
                "natural_language_turn reached max tool calls"
            );
            format!(
                "已达到单回合最多 {} 次工具调用限制，请继续下一轮。",
                self.max_tool_calls_per_turn
            )
        } else {
            warn!(
                turn_id = turn_id,
                planner_rounds = planner_rounds,
                max_rounds = self.max_planner_rounds_per_turn,
                skill_shell_misfire_count = skill_shell_misfire_count,
                skill_workflow_shortcut_used = saw_git_status_short && saw_git_diff_stat,
                diff_strategy = diff_strategy,
                "natural_language_turn reached max planner rounds"
            );
            format!(
                "已达到单回合最多 {} 次规划轮次限制，请继续下一轮。",
                self.max_planner_rounds_per_turn
            )
        };
        self.push_event(
            events,
            Event::ResponseError {
                turn_id,
                code: "turn_limit_reached".to_string(),
                message: message.clone(),
                retryable: true,
            },
        );
        if matches!(
            turn_engine.phase(),
            crate::agent::turn_engine::TurnEnginePhase::Planning
        ) {
            turn_engine.on_failed();
        }
    }
}
