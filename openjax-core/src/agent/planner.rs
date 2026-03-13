use std::collections::{BTreeMap, HashSet};
use std::time::Instant;
use tokio::task::JoinSet;

use openjax_protocol::Event;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agent::decision::{
    DecisionJsonStreamParser, NormalizedToolCall, normalize_model_decision, normalize_tool_calls,
    parse_model_decision, parse_model_decision_v2,
};
use crate::agent::prompt::{
    build_final_response_prompt, build_json_repair_prompt, build_planner_input, truncate_for_prompt,
};
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::tool_policy::{
    approval_rejected_stop_message, approval_timed_out_stop_message, duplicate_skip_abort_message,
    duplicate_tool_call_warning, is_approval_blocking_error,
    should_abort_on_consecutive_duplicate_skips,
};
use crate::agent::turn_engine::TurnEngine;
use crate::model::{ModelRequest, ModelStage};
use crate::{
    Agent, FinalResponseMode, MAX_CONSECUTIVE_DUPLICATE_SKIPS,
    SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS, tools,
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
        let mut skill_shell_misfire_count = 0usize;
        let mut saw_git_status_short = false;
        let mut saw_git_diff_stat = false;
        let mut diff_strategy = "none";
        let mut turn_engine = TurnEngine::new();

        debug!(
            turn_id = turn_id,
            user_input_len = user_input.len(),
            "natural_language_turn started"
        );

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
                        turn_engine.on_failed();
                        self.record_history("assistant", message);
                        return;
                    }
                };

                let args = decision.args.clone().unwrap_or_default();
                if let Some(cmd) = args.get("cmd")
                    && looks_like_skill_trigger_shell_command(cmd)
                {
                    skill_shell_misfire_count = skill_shell_misfire_count.saturating_add(1);
                }
                if let Some(cmd) = args.get("cmd") {
                    if is_git_status_short(cmd) {
                        saw_git_status_short = true;
                    }
                    if is_git_diff_stat(cmd) {
                        saw_git_diff_stat = true;
                    }
                    if let Some(next_strategy) = detect_diff_strategy(cmd) {
                        diff_strategy = merge_diff_strategy(diff_strategy, next_strategy);
                    }
                }

                if let Some(message) =
                    apply_patch_read_guard.block_user_message_for_tool(&tool_name)
                {
                    let tool_call_id = Uuid::new_v4().to_string();
                    warn!(
                        turn_id = turn_id,
                        tool_call_id = %tool_call_id,
                        reason = apply_patch_read_guard
                            .block_log_reason_for_tool(&tool_name)
                            .unwrap_or("unknown"),
                        "apply_patch blocked by read-before-repatch guard"
                    );

                    self.push_event(
                        events,
                        Event::ToolCallStarted {
                            turn_id,
                            tool_call_id: tool_call_id.clone(),
                            tool_name: tool_name.clone(),
                            target: extract_tool_target_hint(&tool_name, &args),
                        },
                    );

                    self.record_tool_call(&tool_name, &args, false, message);
                    tool_traces.push(format!(
                        "tool={tool_name}; ok=false; output={}",
                        truncate_for_prompt(
                            message,
                            self.skill_runtime_config.max_diff_chars_for_planner
                        )
                    ));
                    self.push_event(
                        events,
                        Event::ToolCallCompleted {
                            turn_id,
                            tool_call_id: tool_call_id.clone(),
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
                        turn_engine.on_failed();
                        self.record_history("assistant", loop_message);
                        return;
                    }
                    continue;
                }

                let call = tools::ToolCall {
                    name: tool_name.clone(),
                    args: args.clone(),
                };
                let tool_call_id = Uuid::new_v4().to_string();

                let start_time = Instant::now();
                info!(
                    turn_id = turn_id,
                    tool_call_id = %tool_call_id,
                    tool_name = %call.name,
                    args = ?call.args,
                    "tool_call started"
                );

                self.push_event(
                    events,
                    Event::ToolCallStarted {
                        turn_id,
                        tool_call_id: tool_call_id.clone(),
                        tool_name: tool_name.clone(),
                        target: extract_tool_target_hint(&tool_name, &args),
                    },
                );

                match self
                    .execute_tool_with_live_events(turn_id, &tool_call_id, &call, events)
                    .await
                {
                    Ok(outcome) => {
                        let output = outcome.output;
                        let ok = outcome.success;
                        apply_patch_read_guard.on_tool_success(&tool_name);

                        if is_mutating_tool(&tool_name) {
                            // File/content state has changed. Move to a new epoch so read/list
                            // calls with the same args can run again against fresh state.
                            self.state_epoch = self.state_epoch.saturating_add(1);
                        }

                        let duration_ms = start_time.elapsed().as_millis();
                        info!(
                            turn_id = turn_id,
                            tool_call_id = %tool_call_id,
                            tool_name = %tool_name,
                            ok = ok,
                            duration_ms = duration_ms,
                            output_len = output.len(),
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok={ok}; output={}",
                            truncate_for_prompt(
                                &output,
                                self.skill_runtime_config.max_diff_chars_for_planner
                            )
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, ok, &output);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_call_id: tool_call_id.clone(),
                                tool_name: tool_name.to_string(),
                                ok,
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
                            tool_call_id = %tool_call_id,
                            tool_name = %tool_name,
                            ok = false,
                            duration_ms = duration_ms,
                            error = %err_text,
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok=false; output={}",
                            truncate_for_prompt(
                                &err_text,
                                self.skill_runtime_config.max_diff_chars_for_planner
                            )
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, false, &err_text);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_call_id: tool_call_id.clone(),
                                tool_name: tool_name.to_string(),
                                ok: false,
                                output: err_text.to_string(),
                            },
                        );
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                        if is_approval_blocking_error(&err_text) {
                            let stop_message =
                                if err_text.to_ascii_lowercase().contains("approval timed out") {
                                    approval_timed_out_stop_message()
                                } else {
                                    approval_rejected_stop_message()
                                };
                            self.push_event(
                                events,
                                Event::AssistantMessage {
                                    turn_id,
                                    content: stop_message.clone(),
                                },
                            );
                            turn_engine.on_failed();
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

    async fn request_planner_model_output(
        &mut self,
        turn_id: u64,
        planner_request: &ModelRequest,
        emit_live_final_deltas: bool,
        events: &mut Vec<Event>,
    ) -> anyhow::Result<PlannerStreamResult> {
        let started_at = Instant::now();
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_future = self
            .model_client
            .complete_stream(planner_request, Some(delta_tx));
        tokio::pin!(stream_future);

        let mut parser = DecisionJsonStreamParser::new();
        let mut streamed_message = String::new();
        let mut response_started = false;
        let mut ttft_logged = false;

        let on_delta = |delta: String,
                        parser: &mut DecisionJsonStreamParser,
                        streamed_message: &mut String,
                        response_started: &mut bool,
                        ttft_logged: &mut bool,
                        events: &mut Vec<Event>| {
            if delta.is_empty() {
                return;
            }
            let chunk = parser.push_chunk(&delta);
            if !emit_live_final_deltas || chunk.message_delta.is_empty() {
                return;
            }
            if !*response_started {
                *response_started = true;
                self.push_event(
                    events,
                    Event::ResponseStarted {
                        turn_id,
                        stream_source: openjax_protocol::StreamSource::ModelLive,
                    },
                );
            }
            if !*ttft_logged {
                *ttft_logged = true;
                info!(
                    turn_id = turn_id,
                    planner_stream_ttft_ms = started_at.elapsed().as_millis(),
                    "planner_stream_ttft"
                );
            }
            for ch in chunk.message_delta.chars() {
                let delta = ch.to_string();
                streamed_message.push(ch);
                self.push_event(
                    events,
                    Event::ResponseTextDelta {
                        turn_id,
                        content_delta: delta,
                        stream_source: openjax_protocol::StreamSource::ModelLive,
                    },
                );
            }
        };

        let stream_result = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        on_delta(
                            delta,
                            &mut parser,
                            &mut streamed_message,
                            &mut response_started,
                            &mut ttft_logged,
                            events,
                        );
                    }
                }
                result = &mut stream_future => {
                    break result;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            on_delta(
                delta,
                &mut parser,
                &mut streamed_message,
                &mut response_started,
                &mut ttft_logged,
                events,
            );
        }

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
        })
    }

    pub(crate) async fn stream_final_assistant_reply(
        &mut self,
        turn_id: u64,
        user_input: &str,
        tool_traces: &[String],
        seed_message: &str,
        events: &mut Vec<Event>,
    ) -> (String, bool) {
        let prompt = build_final_response_prompt(user_input, tool_traces, seed_message);
        self.apply_rate_limit().await;
        let stream_started_at = Instant::now();
        let mut ttft_logged = false;
        self.push_event(
            events,
            Event::ResponseStarted {
                turn_id,
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
        );

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_request = ModelRequest::for_stage(ModelStage::FinalWriter, prompt);
        let stream_future = self
            .model_client
            .complete_stream(&stream_request, Some(delta_tx));
        tokio::pin!(stream_future);

        let mut streamed = String::new();
        let result = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta
                        && !delta.is_empty() {
                        if !ttft_logged {
                            ttft_logged = true;
                            info!(
                                turn_id = turn_id,
                                ttft_ms = stream_started_at.elapsed().as_millis(),
                                "final_stream_ttft"
                            );
                        }
                        streamed.push_str(&delta);
                        self.push_event(events, Event::ResponseTextDelta {
                            turn_id,
                            content_delta: delta,
                            stream_source: openjax_protocol::StreamSource::ModelLive,
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
                if !ttft_logged {
                    ttft_logged = true;
                    info!(
                        turn_id = turn_id,
                        ttft_ms = stream_started_at.elapsed().as_millis(),
                        "final_stream_ttft"
                    );
                }
                streamed.push_str(&delta);
                self.push_event(
                    events,
                    Event::ResponseTextDelta {
                        turn_id,
                        content_delta: delta,
                        stream_source: openjax_protocol::StreamSource::ModelLive,
                    },
                );
            }
        }

        match result {
            Ok(response) => {
                let full_text = response.text;
                info!(
                    turn_id = turn_id,
                    output_len = full_text.len(),
                    streamed_len = streamed.len(),
                    "final_writer_request completed"
                );
                let resolved = if streamed.is_empty() {
                    full_text
                } else if full_text.is_empty() || full_text == streamed {
                    streamed
                } else {
                    full_text
                };
                self.push_event(
                    events,
                    Event::ResponseCompleted {
                        turn_id,
                        content: resolved.clone(),
                        stream_source: openjax_protocol::StreamSource::ModelLive,
                    },
                );
                (resolved, true)
            }
            Err(err) => {
                warn!(turn_id = turn_id, error = %err, "final response streaming failed; fallback to planner message");
                self.push_event(
                    events,
                    Event::ResponseError {
                        turn_id,
                        code: "final_writer_stream_failed".to_string(),
                        message: err.to_string(),
                        retryable: true,
                    },
                );
                (seed_message.to_string(), false)
            }
        }
    }

    fn emit_synthetic_response_deltas(
        &mut self,
        turn_id: u64,
        message: &str,
        events: &mut Vec<Event>,
    ) {
        if message.is_empty() {
            return;
        }

        let mut chunk = String::new();
        let mut chunk_len = 0usize;

        for ch in message.chars() {
            chunk.push(ch);
            chunk_len += 1;
            if chunk_len >= SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS {
                self.push_event(
                    events,
                    Event::ResponseTextDelta {
                        turn_id,
                        content_delta: chunk.clone(),
                        stream_source: openjax_protocol::StreamSource::Synthetic,
                    },
                );
                chunk.clear();
                chunk_len = 0;
            }
        }

        if !chunk.is_empty() {
            self.push_event(
                events,
                Event::ResponseTextDelta {
                    turn_id,
                    content_delta: chunk,
                    stream_source: openjax_protocol::StreamSource::Synthetic,
                },
            );
        }
    }

    async fn execute_tool_batch_calls(
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

#[derive(Debug, Default, Clone)]
struct PlannerStreamResult {
    model_output: String,
    streamed_message: String,
    live_streamed: bool,
}

fn extract_tool_target_hint(
    tool_name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let keys: &[&str] = match tool_name {
        "read_file" | "apply_patch" | "edit_file_range" | "write_file" => {
            &["file_path", "path", "filepath"]
        }
        "disk_usage" => &["path"],
        "shell" | "exec_command" => &["cmd", "command"],
        _ => return None,
    };
    keys.iter().find_map(|k| args.get(*k).cloned())
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

fn summarize_log_preview_json(text: &str, limit: usize) -> String {
    let (preview, truncated) = summarize_log_preview(text, limit);
    serde_json::json!({
        "model_output": preview,
        "truncated": truncated,
    })
    .to_string()
}

fn looks_like_skill_trigger_shell_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') {
        return false;
    }
    if trimmed.contains(char::is_whitespace) {
        return false;
    }
    trimmed[1..].chars().all(|ch| ch != '/')
}

fn is_git_status_short(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized == "git status --short" || normalized == "git status -s"
}

fn is_git_diff_stat(command: &str) -> bool {
    command
        .trim()
        .to_ascii_lowercase()
        .contains("git diff --stat")
}

fn detect_diff_strategy(command: &str) -> Option<&'static str> {
    let normalized = command.trim().to_ascii_lowercase();
    if !normalized.contains("git diff") {
        return None;
    }
    if normalized.contains("git diff --stat") {
        return Some("stat_only");
    }
    if normalized.contains("git diff --staged")
        || normalized.contains("git diff --cached")
        || normalized.contains("git diff -- ")
    {
        return Some("targeted");
    }
    Some("full")
}

fn merge_diff_strategy(current: &str, next: &str) -> &'static str {
    fn rank(value: &str) -> u8 {
        match value {
            "stat_only" => 1,
            "targeted" => 2,
            "full" => 3,
            _ => 0,
        }
    }
    if rank(next) >= rank(current) {
        match next {
            "stat_only" => "stat_only",
            "targeted" => "targeted",
            "full" => "full",
            _ => "none",
        }
    } else {
        match current {
            "stat_only" => "stat_only",
            "targeted" => "targeted",
            "full" => "full",
            _ => "none",
        }
    }
}
