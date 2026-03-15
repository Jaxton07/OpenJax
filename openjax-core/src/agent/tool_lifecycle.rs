use std::collections::HashMap;

use openjax_protocol::Event;

use crate::Agent;
use crate::agent::planner_utils::{
    extract_tool_target_hint, tool_args_delta_payload, tool_failure_code, tool_failure_retryable,
};

impl Agent {
    pub(super) fn emit_tool_call_started_sequence(
        &self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        args: &HashMap<String, String>,
        progress_message: &str,
        events: &mut Vec<Event>,
    ) {
        self.push_event(
            events,
            Event::ToolCallStarted {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                target: extract_tool_target_hint(tool_name, args),
            },
        );
        if let Some(args_delta) = tool_args_delta_payload(args) {
            self.push_event(
                events,
                Event::ToolCallArgsDelta {
                    turn_id,
                    tool_call_id: tool_call_id.to_string(),
                    tool_name: tool_name.to_string(),
                    args_delta,
                },
            );
        }
        self.push_event(
            events,
            Event::ToolCallReady {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
            },
        );
        self.push_event(
            events,
            Event::ToolCallProgress {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                progress_message: progress_message.to_string(),
            },
        );
    }

    pub(super) fn emit_tool_call_failed(
        &self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        error_text: &str,
        events: &mut Vec<Event>,
    ) {
        self.push_event(
            events,
            Event::ToolCallFailed {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                code: tool_failure_code(error_text).to_string(),
                message: error_text.to_string(),
                retryable: tool_failure_retryable(error_text),
            },
        );
    }

    pub(super) fn emit_tool_call_completed(
        &self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        ok: bool,
        output: &str,
        events: &mut Vec<Event>,
    ) {
        self.push_event(
            events,
            Event::ToolCallCompleted {
                turn_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                ok,
                output: output.to_string(),
            },
        );
    }
}
