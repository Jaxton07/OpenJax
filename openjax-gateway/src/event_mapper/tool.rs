use openjax_protocol::Event;
use serde_json::json;

use super::CoreEventMapping;

pub fn map(event: &Event) -> Option<CoreEventMapping> {
    match event {
        Event::ToolCallsProposed {
            turn_id,
            tool_calls,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_calls_proposed",
            payload: json!({ "tool_calls": tool_calls }),
            stream_source: None,
        }),
        Event::ToolBatchCompleted {
            turn_id,
            total,
            succeeded,
            failed,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_batch_completed",
            payload: json!({ "total": total, "succeeded": succeeded, "failed": failed }),
            stream_source: None,
        }),
        Event::ToolCallStarted {
            turn_id,
            tool_call_id,
            tool_name,
            target,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_call_started",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "target": target, "display_name": display_name }),
            stream_source: None,
        }),
        Event::ToolCallCompleted {
            turn_id,
            tool_call_id,
            tool_name,
            ok,
            output,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_call_completed",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "ok": ok, "output": output, "display_name": display_name }),
            stream_source: None,
        }),
        Event::ToolCallArgsDelta {
            turn_id,
            tool_call_id,
            tool_name,
            args_delta,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_args_delta",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "args_delta": args_delta, "display_name": display_name }),
            stream_source: None,
        }),
        Event::ToolCallReady {
            turn_id,
            tool_call_id,
            tool_name,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_call_ready",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "display_name": display_name }),
            stream_source: None,
        }),
        Event::ToolCallProgress {
            turn_id,
            tool_call_id,
            tool_name,
            progress_message,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_call_progress",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "progress_message": progress_message, "display_name": display_name }),
            stream_source: None,
        }),
        Event::ToolCallFailed {
            turn_id,
            tool_call_id,
            tool_name,
            code,
            message,
            retryable,
            display_name,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "tool_call_failed",
            payload: json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "code": code, "message": message, "retryable": retryable, "display_name": display_name }),
            stream_source: None,
        }),
        _ => None,
    }
}
