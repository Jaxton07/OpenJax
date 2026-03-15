use std::collections::BTreeMap;

use openjax_protocol::Event;
use tracing::{debug, info, warn};

use crate::agent::decision::{
    normalize_model_decision, normalize_tool_calls, parse_model_decision, parse_model_decision_v2,
};
use crate::agent::planner_utils::{summarize_log_preview, summarize_log_preview_json};
use crate::agent::prompt::{build_json_repair_prompt, build_planner_input};
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::turn_engine::TurnEngine;
use crate::model::{ModelRequest, ModelStage};
use crate::{Agent, FinalResponseMode};

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

        if self.direct_provider_stream {
            info!(
                turn_id = turn_id,
                "direct_provider_stream enabled; bypass planner/tool decision pipeline"
            );
            turn_engine.on_response_started();
            let (message, ok) = self
                .stream_direct_provider_reply(turn_id, user_input, events)
                .await;
            self.push_event(
                events,
                Event::AssistantMessage {
                    turn_id,
                    content: message.clone(),
                },
            );
            if ok {
                turn_engine.on_completed();
            } else {
                turn_engine.on_failed();
            }
            self.record_history("assistant", message);
            return;
        }

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
            let planner_input = build_planner_input(
                user_input,
                &self.history,
                &tool_traces,
                remaining,
                &skills_context,
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
                .request_planner_model_output(
                    turn_id,
                    &planner_request,
                    self.final_response_mode == FinalResponseMode::PlannerOnly,
                    events,
                )
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
                    let message = format!("[model error] {err}");
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        },
                    );
                    turn_engine.on_failed();
                    self.record_history("assistant", message);
                    return;
                }
            };
            let model_output = planner_stream.model_output;

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

            if self.tool_batch_v2_enabled
                && let Some(v2) = parse_model_decision_v2(&model_output)
                && v2.action.eq_ignore_ascii_case("tool_batch")
            {
                let mut normalized_calls = normalize_tool_calls(&v2.tool_calls);
                if normalized_calls.is_empty() {
                    let message = "[model error] tool_batch missing valid tool_calls".to_string();
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
                if normalized_calls.len() > remaining {
                    normalized_calls.truncate(remaining);
                }
                let proposals = normalized_calls
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
                let executed = self
                    .execute_tool_batch_calls(
                        turn_id,
                        normalized_calls,
                        events,
                        &mut tool_traces,
                        &mut apply_patch_read_guard,
                        &mut turn_engine,
                    )
                    .await;
                executed_count += executed;
                consecutive_duplicate_skips = 0;
                turn_engine.on_response_resumed();
                continue;
            }

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

                let repair_request = ModelRequest::for_stage(ModelStage::Planner, repair_prompt);
                let repaired_output = match self.model_client.complete(&repair_request).await {
                    Ok(output) => output.text,
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
                        turn_engine.on_failed();
                        self.record_history("assistant", message);
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
                    turn_engine.on_failed();
                    self.record_history("assistant", model_output);
                    return;
                }
            };

            let action = decision.action.to_ascii_lowercase();

            info!(
                turn_id = turn_id,
                action = %action,
                tool = ?decision.tool,
                "model_decision"
            );

            debug!(
                turn_id = turn_id,
                args = ?decision.args,
                message = ?decision.message,
                "model_decision_payload"
            );

            if action == "final" {
                let seed_message = decision
                    .message
                    .unwrap_or_else(|| "任务已完成。".to_string());
                info!(
                    turn_id = turn_id,
                    phase = "completed",
                    action = "final",
                    final_response_mode = self.final_response_mode.as_str(),
                    skill_shell_misfire_count = skill_shell_misfire_count,
                    skill_workflow_shortcut_used = saw_git_status_short && saw_git_diff_stat,
                    diff_strategy = diff_strategy,
                    "natural_language_turn completed"
                );
                let message = if self.final_response_mode == FinalResponseMode::FinalWriter {
                    turn_engine.on_response_started();
                    let (streamed, live_streamed) = self
                        .stream_final_assistant_reply(
                            turn_id,
                            user_input,
                            &tool_traces,
                            &seed_message,
                            events,
                        )
                        .await;
                    if live_streamed {
                        streamed
                    } else {
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
                    }
                } else {
                    info!(
                        turn_id = turn_id,
                        mode = self.final_response_mode.as_str(),
                        "final_writer_request skipped"
                    );
                    if planner_stream.live_streamed {
                        turn_engine.on_response_started();
                        self.push_event(
                            events,
                            Event::ResponseCompleted {
                                turn_id,
                                content: seed_message.clone(),
                                stream_source: openjax_protocol::StreamSource::ModelLive,
                            },
                        );
                        if planner_stream.streamed_message.is_empty() {
                            seed_message.clone()
                        } else {
                            planner_stream.streamed_message.clone()
                        }
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
                    }
                };
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
                turn_engine.on_completed();
                self.record_history("assistant", message);
                return;
            }

            if action == "tool" {
                let should_continue = self
                    .handle_tool_action(
                        turn_id,
                        &decision,
                        events,
                        &mut tool_traces,
                        &mut apply_patch_read_guard,
                        &mut consecutive_duplicate_skips,
                        &mut executed_count,
                        &mut turn_engine,
                        &mut skill_shell_misfire_count,
                        &mut saw_git_status_short,
                        &mut saw_git_diff_stat,
                        &mut diff_strategy,
                    )
                    .await;
                if should_continue {
                    continue;
                }
                return;
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
            turn_engine.on_failed();
            self.record_history("assistant", message);
            return;
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
            Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            },
        );
        if matches!(
            turn_engine.phase(),
            crate::agent::turn_engine::TurnEnginePhase::Planning
        ) {
            turn_engine.on_failed();
        }
        self.record_history("assistant", message);
    }
}
