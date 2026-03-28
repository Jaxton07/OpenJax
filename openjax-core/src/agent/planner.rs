use std::collections::BTreeMap;

use openjax_protocol::Event;
use tracing::{debug, info, warn};

use crate::Agent;
use crate::agent::planner_tool_action::NativeToolExecOutcome;
use crate::agent::planner_utils::summarize_log_preview;
use crate::agent::prompt::{
    build_system_prompt, build_turn_messages, refresh_loop_recovery_in_messages,
};
use crate::agent::tool_guard::ApplyPatchReadGuard;
use crate::agent::turn_engine::TurnEngine;
use crate::logger::AFTER_DISPATCH_LOG_TARGET;
use crate::model::{
    AssistantContentBlock, ConversationMessage, ModelRequest, ModelStage, UserContentBlock,
};

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
        let system_prompt = build_system_prompt(&skills_context);
        let mut messages = build_turn_messages(
            user_input,
            &self.history,
            self.loop_detector.recovery_prompt(),
        );

        info!(
            turn_id = turn_id,
            skills_selected = selected_skills.len(),
            skills_prompt_chars = skills_context.chars().count(),
            "skills_context prepared"
        );

        while executed_count < self.max_tool_calls_per_turn
            && planner_rounds < self.max_planner_rounds_per_turn
        {
            refresh_loop_recovery_in_messages(
                &mut messages,
                user_input,
                self.loop_detector.recovery_prompt(),
            );
            let remaining = self.max_tool_calls_per_turn - executed_count;

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
                message_count = messages.len(),
                "model_request started"
            );
            planner_rounds += 1;

            let planner_request = ModelRequest {
                stage: ModelStage::Planner,
                messages: messages.clone(),
                system_prompt: Some(system_prompt.clone()),
                tools: self.tools.tool_specs(),
                options: Default::default(),
            };
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
            // 更新 last_input_tokens（来自流式 usage 或 fallback usage）
            if let Some(ref usage) = planner_stream.usage
                && let Some(tokens) = usage.input_tokens
            {
                self.last_input_tokens = Some(tokens);
            }
            // 检查是否需要自动压缩
            self.check_and_auto_compact(turn_id, events).await;
            let live_streamed = planner_stream.live_streamed;
            let response = planner_stream.response;
            let streamed_text = planner_stream.streamed_text;

            info!(
                turn_id = turn_id,
                content_blocks = response.content.len(),
                tool_uses = response.tool_uses().len(),
                "model_request completed"
            );

            debug!(
                turn_id = turn_id,
                response = ?response.content,
                "model_response_content"
            );

            messages.push(ConversationMessage::Assistant(response.content.clone()));

            if !response.has_tool_use() {
                tracing::info!(
                    turn_id = turn_id,
                    flow_prefix = FLOW_TRACE_PREFIX,
                    flow_node = "planner.final.emit",
                    flow_route = "final",
                    flow_next = "frontend.response_completed",
                    live_streamed = live_streamed,
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

                let seed_message = response.text();
                let message = if live_streamed {
                    turn_engine.on_response_started();
                    let completed_content = if streamed_text.is_empty() {
                        seed_message.clone()
                    } else {
                        streamed_text
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
                    skill_shell_misfire_count = skill_shell_misfire_count,
                    skill_workflow_shortcut_used = saw_git_status_short && saw_git_diff_stat,
                    diff_strategy = diff_strategy,
                    "final_response"
                );
                turn_engine.on_completed();
                self.commit_turn(user_input.to_string(), tool_traces, message);
                return;
            }

            let tool_uses = response
                .content
                .iter()
                .filter_map(|block| match block {
                    AssistantContentBlock::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            tracing::info!(
                turn_id = turn_id,
                flow_prefix = FLOW_TRACE_PREFIX,
                flow_node = "planner.dispatch_consume",
                flow_route = "tool_use",
                flow_next = "planner.tool_action.execute",
                tool_calls = tool_uses.len(),
                "flow_trace"
            );
            log_after_dispatch_step(
                turn_id,
                "planner.dispatch_consume",
                "tool_use",
                "planner.tool_action.execute",
                None,
                None,
            );
            let proposals = tool_uses
                .iter()
                .map(|(id, name, input)| openjax_protocol::ToolCallProposal {
                    tool_call_id: id.clone(),
                    tool_name: name.clone(),
                    arguments: value_to_btreemap(input),
                    depends_on: Vec::new(),
                    concurrency_group: None,
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
            let mut tool_result_blocks = Vec::new();
            let mut succeeded = 0u32;
            let mut failed = 0u32;
            let executable_count = tool_uses.len().min(remaining);
            let dropped_tool_uses = if tool_uses.len() > executable_count {
                tool_uses[executable_count..].to_vec()
            } else {
                Vec::new()
            };

            for (tool_call_id, tool_name, _) in &dropped_tool_uses {
                let message = format!(
                    "tool call skipped because the turn only allows {} tool calls",
                    self.max_tool_calls_per_turn
                );
                self.push_event(
                    events,
                    Event::ToolCallFailed {
                        turn_id,
                        tool_call_id: tool_call_id.clone(),
                        tool_name: tool_name.clone(),
                        code: "tool_call_budget_exhausted".to_string(),
                        message: message.clone(),
                        retryable: true,
                        display_name: self.tools.display_name_for(tool_name),
                    },
                );
                self.emit_tool_call_completed(
                    turn_id,
                    tool_call_id,
                    tool_name,
                    false,
                    &message,
                    events,
                );
                failed = failed.saturating_add(1);
            }

            for (tool_call_id, tool_name, input) in tool_uses.into_iter().take(executable_count) {
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
                match self
                    .execute_native_tool_call(turn_id, &tool_call_id, &tool_name, &input, &mut ctx)
                    .await
                {
                    NativeToolExecOutcome::Result { model_content, ok } => {
                        if ok {
                            succeeded = succeeded.saturating_add(1);
                        } else {
                            failed = failed.saturating_add(1);
                        }
                        tool_result_blocks.push(UserContentBlock::ToolResult {
                            tool_use_id: tool_call_id,
                            content: model_content,
                            is_error: !ok,
                        });
                    }
                    NativeToolExecOutcome::Aborted => {
                        return;
                    }
                }
            }

            self.push_event(
                events,
                Event::ToolBatchCompleted {
                    turn_id,
                    total: (executable_count + dropped_tool_uses.len()) as u32,
                    succeeded,
                    failed,
                },
            );
            if !tool_result_blocks.is_empty() {
                messages.push(ConversationMessage::User(tool_result_blocks));
            }
            turn_engine.on_response_resumed();
            continue;
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

fn value_to_btreemap(input: &serde_json::Value) -> BTreeMap<String, String> {
    let mut arguments = BTreeMap::new();
    let serde_json::Value::Object(map) = input else {
        return arguments;
    };
    for (key, value) in map {
        let stringified = match value {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        arguments.insert(key.clone(), stringified);
    }
    arguments
}
