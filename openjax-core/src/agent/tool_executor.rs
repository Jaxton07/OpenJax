use std::collections::HashMap;
use std::time::Instant;

use openjax_protocol::Event;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agent::loop_detector::LoopSignal;
use crate::agent::planner::ToolActionContext;
use crate::agent::planner_tool_action::NativeToolExecOutcome;
use crate::agent::planner_utils::is_mutating_tool;
use crate::agent::tool_lifecycle::ToolCallCompletedFields;
use crate::agent::tool_policy::{
    approval_rejected_stop_message, approval_timed_out_stop_message, is_approval_blocking_error,
};
use crate::agent::tool_projection::tool_trace;
use crate::{Agent, tools};

impl Agent {
    pub(super) async fn execute_native_tool_call_body(
        &mut self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        args: HashMap<String, String>,
        ctx: &mut ToolActionContext<'_>,
    ) -> NativeToolExecOutcome {
        let call = tools::ToolCall {
            name: tool_name.to_string(),
            args: args.clone(),
        };
        let start_time = Instant::now();
        info!(
            turn_id = turn_id,
            tool_call_id = %tool_call_id,
            tool_name = %call.name,
            args = ?call.args,
            "tool_call started"
        );

        self.push_event(
            ctx.events,
            Event::ToolCallProgress {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                progress_message: "executing".to_string(),
                display_name: self.tools.display_name_for(tool_name),
            },
        );

        match self
            .execute_tool_with_live_events(turn_id, tool_call_id, &call, ctx.events)
            .await
        {
            Ok(outcome) => {
                let model_content = outcome.model_content;
                let output = outcome.display_output;
                let shell_metadata = outcome.shell_metadata;
                let ok = outcome.success;

                if is_mutating_tool(tool_name) {
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
                ctx.tool_traces.push(tool_trace(
                    tool_name,
                    ok,
                    &output,
                    self.skill_runtime_config.max_diff_chars_for_planner,
                ));

                let signal = self.loop_detector.check_and_advance(
                    tool_name,
                    &serde_json::to_string(&args).unwrap_or_default(),
                );
                match signal {
                    LoopSignal::Warned => {
                        info!(turn_id, tool_name, "loop_detected: soft interrupt");
                        self.push_event(
                            ctx.events,
                            Event::LoopWarning {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                consecutive_count: self.loop_detector.warn_threshold(),
                            },
                        );
                    }
                    LoopSignal::Halt => {
                        warn!(
                            turn_id,
                            tool_name, "loop_detected: hard halt after recovery failure"
                        );
                        self.push_event(
                            ctx.events,
                            Event::ResponseError {
                                turn_id,
                                code: "loop_halt".to_string(),
                                message: "检测到持续重复调用，已强制终止本回合。".to_string(),
                                retryable: true,
                            },
                        );
                        ctx.turn_engine.on_failed();
                        return NativeToolExecOutcome::Aborted;
                    }
                    LoopSignal::None => {}
                }

                self.record_tool_call(tool_name, &args, ok, &output);
                self.emit_tool_call_completed_with_fields(
                    ToolCallCompletedFields {
                        turn_id,
                        tool_call_id,
                        tool_name,
                        ok,
                        output: &output,
                        shell_metadata,
                    },
                    ctx.events,
                );
                *ctx.executed_count += 1;
                *ctx.consecutive_duplicate_skips = 0;
                NativeToolExecOutcome::Result { model_content, ok }
            }
            Err(err) => {
                let duration_ms = start_time.elapsed().as_millis();
                let err_text = err.to_string();
                info!(
                    turn_id = turn_id,
                    tool_call_id = %tool_call_id,
                    tool_name = %tool_name,
                    ok = false,
                    duration_ms = duration_ms,
                    error = %err_text,
                    "tool_call completed"
                );
                ctx.tool_traces.push(tool_trace(
                    tool_name,
                    false,
                    &err_text,
                    self.skill_runtime_config.max_diff_chars_for_planner,
                ));

                self.record_tool_call(tool_name, &args, false, &err_text);
                self.emit_tool_call_failed(turn_id, tool_call_id, tool_name, &err_text, ctx.events);
                self.emit_tool_call_completed(
                    turn_id,
                    tool_call_id,
                    tool_name,
                    false,
                    &err_text,
                    ctx.events,
                );
                *ctx.executed_count += 1;
                *ctx.consecutive_duplicate_skips = 0;
                if is_approval_blocking_error(&err_text) {
                    let stop_message = if err_text.to_ascii_lowercase().contains("approval timed out")
                    {
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
                    return NativeToolExecOutcome::Aborted;
                }
                NativeToolExecOutcome::Result {
                    model_content: err_text,
                    ok: false,
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(super) async fn execute_legacy_tool_action_body(
        &mut self,
        turn_id: u64,
        tool_name: String,
        args: HashMap<String, String>,
        ctx: &mut ToolActionContext<'_>,
    ) -> bool {
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
                let output = outcome.display_output;
                let ok = outcome.success;

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
                ctx.tool_traces.push(tool_trace(
                    &tool_name,
                    ok,
                    &output,
                    self.skill_runtime_config.max_diff_chars_for_planner,
                ));

                let signal = self.loop_detector.check_and_advance(
                    &tool_name,
                    &serde_json::to_string(&args).unwrap_or_default(),
                );
                match signal {
                    LoopSignal::Warned => {
                        info!(turn_id, tool_name, "loop_detected: soft interrupt");
                        self.push_event(
                            ctx.events,
                            Event::LoopWarning {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                consecutive_count: self.loop_detector.warn_threshold(),
                            },
                        );
                    }
                    LoopSignal::Halt => {
                        warn!(
                            turn_id,
                            tool_name, "loop_detected: hard halt after recovery failure"
                        );
                        self.push_event(
                            ctx.events,
                            Event::ResponseError {
                                turn_id,
                                code: "loop_halt".to_string(),
                                message: "检测到持续重复调用，已强制终止本回合。".to_string(),
                                retryable: true,
                            },
                        );
                        ctx.turn_engine.on_failed();
                        return false;
                    }
                    LoopSignal::None => {}
                }

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
                info!(
                    turn_id = turn_id,
                    tool_call_id = %tool_call_id,
                    tool_name = %tool_name,
                    ok = false,
                    duration_ms = duration_ms,
                    error = %err_text,
                    "tool_call completed"
                );
                ctx.tool_traces.push(tool_trace(
                    &tool_name,
                    false,
                    &err_text,
                    self.skill_runtime_config.max_diff_chars_for_planner,
                ));

                self.record_tool_call(&tool_name, &args, false, &err_text);
                self.emit_tool_call_failed(
                    turn_id,
                    &tool_call_id,
                    &tool_name,
                    &err_text,
                    ctx.events,
                );
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
                    let stop_message = if err_text.to_ascii_lowercase().contains("approval timed out")
                    {
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
