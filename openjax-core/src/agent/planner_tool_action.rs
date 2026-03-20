use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agent::decision::ModelDecision;
use crate::agent::planner::ToolActionContext;
use crate::agent::planner_utils::{
    detect_diff_strategy, is_git_diff_stat, is_git_status_short, is_mutating_tool,
    looks_like_skill_trigger_shell_command, merge_diff_strategy,
};
use crate::agent::prompt::truncate_for_prompt;
use crate::agent::tool_policy::{
    approval_rejected_stop_message, approval_timed_out_stop_message, duplicate_skip_abort_message,
    duplicate_tool_call_warning, is_approval_blocking_error,
    should_abort_on_consecutive_duplicate_skips,
};
use crate::{Agent, MAX_CONSECUTIVE_DUPLICATE_SKIPS, tools};

impl Agent {
    pub(super) async fn handle_tool_action(
        &mut self,
        turn_id: u64,
        decision: &ModelDecision,
        ctx: &mut ToolActionContext<'_>,
    ) -> bool {
        let tool_name = match decision.tool.clone() {
            Some(name) if !name.trim().is_empty() => name,
            _ => {
                warn!(turn_id = turn_id, "model_decision missing tool name");
                self.push_event(
                    ctx.events,
                    Event::ResponseError {
                        turn_id,
                        code: "model_invalid_tool".to_string(),
                        message: "[model error] tool action missing tool name".to_string(),
                        retryable: false,
                    },
                );
                ctx.turn_engine.on_failed();
                return false;
            }
        };

        let args = decision.args.clone().unwrap_or_default();
        if let Some(cmd) = args.get("cmd")
            && looks_like_skill_trigger_shell_command(cmd)
        {
            *ctx.skill_shell_misfire_count = (*ctx.skill_shell_misfire_count).saturating_add(1);
        }
        if let Some(cmd) = args.get("cmd") {
            if is_git_status_short(cmd) {
                *ctx.saw_git_status_short = true;
            }
            if is_git_diff_stat(cmd) {
                *ctx.saw_git_diff_stat = true;
            }
            if let Some(next_strategy) = detect_diff_strategy(cmd) {
                *ctx.diff_strategy = merge_diff_strategy(ctx.diff_strategy, next_strategy);
            }
        }

        if let Some(message) = ctx.apply_patch_read_guard.block_user_message_for_tool(&tool_name) {
            let tool_call_id = Uuid::new_v4().to_string();
            warn!(
                turn_id = turn_id,
                tool_call_id = %tool_call_id,
                reason = ctx.apply_patch_read_guard
                    .block_log_reason_for_tool(&tool_name)
                    .unwrap_or("unknown"),
                "apply_patch blocked by read-before-repatch guard"
            );

            self.emit_tool_call_started_sequence(
                turn_id,
                &tool_call_id,
                &tool_name,
                &args,
                "executing",
                ctx.events,
            );
            self.push_event(
                ctx.events,
                Event::ToolCallFailed {
                    turn_id,
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    code: "guard_blocked".to_string(),
                    message: message.to_string(),
                    retryable: false,
                },
            );

            self.record_tool_call(&tool_name, &args, false, message);
            ctx.tool_traces.push(format!(
                "tool={tool_name}; ok=false; output={}",
                truncate_for_prompt(
                    message,
                    self.skill_runtime_config.max_diff_chars_for_planner
                )
            ));
            self.emit_tool_call_completed(
                turn_id,
                &tool_call_id,
                &tool_name,
                false,
                message,
                ctx.events,
            );
            *ctx.executed_count += 1;
            *ctx.consecutive_duplicate_skips = 0;
            return true;
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
                ctx.events,
                Event::ResponseError {
                    turn_id,
                    code: "duplicate_tool_call_skipped".to_string(),
                    message: message.clone(),
                    retryable: true,
                },
            );
            ctx.tool_traces.push(format!(
                "tool={tool_name}; ok=skipped_duplicate; args={}; output={}",
                serde_json::to_string(&args).unwrap_or_default(),
                truncate_for_prompt(&message, self.skill_runtime_config.max_diff_chars_for_planner)
            ));
            *ctx.consecutive_duplicate_skips = (*ctx.consecutive_duplicate_skips).saturating_add(1);
            if should_abort_on_consecutive_duplicate_skips(
                *ctx.consecutive_duplicate_skips,
                MAX_CONSECUTIVE_DUPLICATE_SKIPS,
            ) {
                let loop_message = duplicate_skip_abort_message(MAX_CONSECUTIVE_DUPLICATE_SKIPS);
                self.push_event(
                    ctx.events,
                    Event::ResponseError {
                        turn_id,
                        code: "duplicate_tool_call_loop_abort".to_string(),
                        message: loop_message.clone(),
                        retryable: true,
                    },
                );
                ctx.tool_traces.push(format!(
                    "tool={tool_name}; ok=aborted; args={}; output={}",
                    serde_json::to_string(&args).unwrap_or_default(),
                    truncate_for_prompt(&loop_message, self.skill_runtime_config.max_diff_chars_for_planner)
                ));
                ctx.turn_engine.on_failed();
                return false;
            }
            return true;
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

        self.emit_tool_call_started_sequence(
            turn_id,
            &tool_call_id,
            &tool_name,
            &args,
            "executing",
            ctx.events,
        );

        match self
            .execute_tool_with_live_events(turn_id, &tool_call_id, &call, ctx.events)
            .await
        {
            Ok(outcome) => {
                let output = outcome.output;
                let ok = outcome.success;
                ctx.apply_patch_read_guard.on_tool_success(&tool_name);

                if is_mutating_tool(&tool_name) {
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
                ctx.tool_traces.push(trace);

                self.record_tool_call(&tool_name, &args, ok, &output);

                self.emit_tool_call_completed(
                    turn_id,
                    &tool_call_id,
                    &tool_name,
                    ok,
                    &output,
                    ctx.events,
                );
                *ctx.executed_count += 1;
                *ctx.consecutive_duplicate_skips = 0;
            }
            Err(err) => {
                let duration_ms = start_time.elapsed().as_millis();
                let err_text = err.to_string();
                ctx.apply_patch_read_guard.on_tool_failure(&tool_name, &err_text);
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
                ctx.tool_traces.push(trace);

                self.record_tool_call(&tool_name, &args, false, &err_text);
                self.emit_tool_call_failed(turn_id, &tool_call_id, &tool_name, &err_text, ctx.events);
                self.emit_tool_call_completed(
                    turn_id,
                    &tool_call_id,
                    &tool_name,
                    false,
                    &err_text,
                    ctx.events,
                );
                *ctx.executed_count += 1;
                *ctx.consecutive_duplicate_skips = 0;
                if is_approval_blocking_error(&err_text) {
                    let stop_message =
                        if err_text.to_ascii_lowercase().contains("approval timed out") {
                            approval_timed_out_stop_message()
                        } else {
                            approval_rejected_stop_message()
                        };
                    self.push_event(
                        ctx.events,
                        Event::ResponseError {
                            turn_id,
                            code: "approval_blocked".to_string(),
                            message: stop_message,
                            retryable: false,
                        },
                    );
                    ctx.turn_engine.on_failed();
                    return false;
                }
            }
        }

        true
    }
}
