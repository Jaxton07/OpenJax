use std::collections::HashMap;

use openjax_protocol::Event;
use tracing::warn;

use crate::agent::decision::ModelDecision;
use crate::agent::planner::ToolActionContext;
use crate::agent::planner_tool_action::NativeToolExecOutcome;
use crate::agent::tool_policy::{
    duplicate_skip_abort_message, duplicate_tool_call_warning,
    should_abort_on_consecutive_duplicate_skips,
};
use crate::agent::tool_projection::tool_trace_with_args;
use crate::{Agent, MAX_CONSECUTIVE_DUPLICATE_SKIPS};

impl Agent {
    pub(super) fn ensure_native_tool_name(
        &mut self,
        turn_id: u64,
        tool_name: &str,
        ctx: &mut ToolActionContext<'_>,
    ) -> Result<(), NativeToolExecOutcome> {
        if tool_name.trim().is_empty() {
            warn!(turn_id = turn_id, "native tool call missing tool name");
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
            return Err(NativeToolExecOutcome::Aborted);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(super) fn ensure_model_decision_tool_name(
        &mut self,
        turn_id: u64,
        decision: &ModelDecision,
        ctx: &mut ToolActionContext<'_>,
    ) -> Option<String> {
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
                return None;
            }
        };
        Some(tool_name)
    }

    pub(super) fn guard_duplicate_native_tool_call(
        &mut self,
        turn_id: u64,
        tool_name: &str,
        args: &HashMap<String, String>,
        ctx: &mut ToolActionContext<'_>,
    ) -> Option<NativeToolExecOutcome> {
        if !self.is_duplicate_tool_call(tool_name, args) {
            return None;
        }
        warn!(
            turn_id = turn_id,
            tool_name = %tool_name,
            args = ?args,
            "duplicate_tool_call detected, skipping"
        );
        let message = duplicate_tool_call_warning(tool_name, args);
        self.push_event(
            ctx.events,
            Event::ResponseError {
                turn_id,
                code: "duplicate_tool_call_skipped".to_string(),
                message: message.clone(),
                retryable: true,
            },
        );
        ctx.tool_traces.push(tool_trace_with_args(
            tool_name,
            "skipped_duplicate",
            args,
            &message,
            self.skill_runtime_config.max_diff_chars_for_planner,
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
            ctx.tool_traces.push(tool_trace_with_args(
                tool_name,
                "aborted",
                args,
                &loop_message,
                self.skill_runtime_config.max_diff_chars_for_planner,
            ));
            ctx.turn_engine.on_failed();
            return Some(NativeToolExecOutcome::Aborted);
        }

        Some(NativeToolExecOutcome::Result {
            model_content: message,
            ok: false,
        })
    }

    #[allow(dead_code)]
    pub(super) fn guard_duplicate_legacy_tool_call(
        &mut self,
        turn_id: u64,
        tool_name: &str,
        args: &HashMap<String, String>,
        ctx: &mut ToolActionContext<'_>,
    ) -> Option<bool> {
        if !self.is_duplicate_tool_call(tool_name, args) {
            return None;
        }
        warn!(
            turn_id = turn_id,
            tool_name = %tool_name,
            args = ?args,
            "duplicate_tool_call detected, skipping"
        );
        let message = duplicate_tool_call_warning(tool_name, args);
        self.push_event(
            ctx.events,
            Event::ResponseError {
                turn_id,
                code: "duplicate_tool_call_skipped".to_string(),
                message: message.clone(),
                retryable: true,
            },
        );
        ctx.tool_traces.push(tool_trace_with_args(
            tool_name,
            "skipped_duplicate",
            args,
            &message,
            self.skill_runtime_config.max_diff_chars_for_planner,
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
            ctx.tool_traces.push(tool_trace_with_args(
                tool_name,
                "aborted",
                args,
                &loop_message,
                self.skill_runtime_config.max_diff_chars_for_planner,
            ));
            ctx.turn_engine.on_failed();
            return Some(false);
        }

        Some(true)
    }
}
