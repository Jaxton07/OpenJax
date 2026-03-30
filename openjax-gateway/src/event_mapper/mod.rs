use openjax_protocol::Event;
use serde_json::{Value, json};

pub mod approval;
pub mod response;
pub mod tool;

#[derive(Debug, Clone)]
pub enum MapResult {
    Mapped(CoreEventMapping),
    IgnoredInternal,
    Unmapped(&'static str),
}

#[derive(Debug, Clone)]
pub struct CoreEventMapping {
    pub core_turn_id: Option<u64>,
    pub event_type: &'static str,
    pub payload: Value,
    pub stream_source: Option<&'static str>,
}

pub fn map_core_event_payload_result(event: &Event) -> MapResult {
    if let Some(mapping) = response::map(event)
        .or_else(|| tool::map(event))
        .or_else(|| approval::map(event))
        .or_else(|| map_misc(event))
    {
        return MapResult::Mapped(mapping);
    }
    classify_unmapped(event)
}

pub fn map_core_event_payload(event: &Event) -> Option<CoreEventMapping> {
    match map_core_event_payload_result(event) {
        MapResult::Mapped(mapping) => Some(mapping),
        MapResult::IgnoredInternal | MapResult::Unmapped(_) => None,
    }
}

fn map_misc(event: &Event) -> Option<CoreEventMapping> {
    match event {
        Event::ShutdownComplete => Some(CoreEventMapping {
            core_turn_id: None,
            event_type: "session_shutdown",
            payload: json!({}),
            stream_source: None,
        }),
        Event::ContextCompacted {
            turn_id,
            compressed_turns,
            retained_turns,
            summary_preview,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "context_compacted",
            payload: json!({
                "compressed_turns": compressed_turns,
                "retained_turns": retained_turns,
                "summary_preview": summary_preview,
            }),
            stream_source: None,
        }),
        Event::ContextUsageUpdated {
            turn_id,
            input_tokens,
            context_window_size,
            ratio,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "context_usage_updated",
            payload: json!({
                "input_tokens": input_tokens,
                "context_window_size": context_window_size,
                "ratio": ratio,
            }),
            stream_source: None,
        }),
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => None,
        _ => None,
    }
}

fn classify_unmapped(event: &Event) -> MapResult {
    match event {
        Event::AgentSpawned { .. }
        | Event::AgentStatusChanged { .. }
        | Event::AssistantMessage { .. }
        | Event::LoopWarning { .. } => MapResult::IgnoredInternal,
        Event::TurnStarted { .. }
        | Event::ToolCallStarted { .. }
        | Event::ToolCallCompleted { .. }
        | Event::ToolCallArgsDelta { .. }
        | Event::ToolCallReady { .. }
        | Event::ToolCallProgress { .. }
        | Event::ToolCallFailed { .. }
        | Event::ResponseStarted { .. }
        | Event::ResponseTextDelta { .. }
        | Event::ReasoningDelta { .. }
        | Event::ToolCallsProposed { .. }
        | Event::ToolBatchCompleted { .. }
        | Event::ResponseResumed { .. }
        | Event::ResponseCompleted { .. }
        | Event::ResponseError { .. }
        | Event::ApprovalRequested { .. }
        | Event::ApprovalResolved { .. }
        | Event::ContextUsageUpdated { .. }
        | Event::ContextCompacted { .. }
        | Event::TurnCompleted { .. }
        | Event::ShutdownComplete => MapResult::Unmapped(event_variant_name(event)),
    }
}

fn event_variant_name(event: &Event) -> &'static str {
    match event {
        Event::TurnStarted { .. } => "turn_started",
        Event::ToolCallStarted { .. } => "tool_call_started",
        Event::ToolCallCompleted { .. } => "tool_call_completed",
        Event::ToolCallArgsDelta { .. } => "tool_call_args_delta",
        Event::ToolCallReady { .. } => "tool_call_ready",
        Event::ToolCallProgress { .. } => "tool_call_progress",
        Event::ToolCallFailed { .. } => "tool_call_failed",
        Event::AssistantMessage { .. } => "assistant_message",
        Event::ResponseStarted { .. } => "response_started",
        Event::ResponseTextDelta { .. } => "response_text_delta",
        Event::ReasoningDelta { .. } => "reasoning_delta",
        Event::ToolCallsProposed { .. } => "tool_calls_proposed",
        Event::ToolBatchCompleted { .. } => "tool_batch_completed",
        Event::ResponseResumed { .. } => "response_resumed",
        Event::ResponseCompleted { .. } => "response_completed",
        Event::LoopWarning { .. } => "loop_warning",
        Event::ResponseError { .. } => "response_error",
        Event::ApprovalRequested { .. } => "approval_requested",
        Event::ApprovalResolved { .. } => "approval_resolved",
        Event::ContextUsageUpdated { .. } => "context_usage_updated",
        Event::AgentSpawned { .. } => "agent_spawned",
        Event::AgentStatusChanged { .. } => "agent_status_changed",
        Event::ContextCompacted { .. } => "context_compacted",
        Event::TurnCompleted { .. } => "turn_completed",
        Event::ShutdownComplete => "shutdown_complete",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use openjax_protocol::{
        AgentStatus, ApprovalKind, Event, ShellExecutionMetadata, ThreadId, ToolCallProposal,
    };

    use super::{MapResult, map_core_event_payload, map_core_event_payload_result};

    fn sample_events_covering_all_variants() -> Vec<Event> {
        vec![
            Event::TurnStarted { turn_id: 1 },
            Event::ToolCallStarted {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                target: Some("cmd".to_string()),
                display_name: Some("Run Shell".to_string()),
            },
            Event::ToolCallCompleted {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                ok: true,
                output: "done".to_string(),
                shell_metadata: Some(ShellExecutionMetadata {
                    result_class: "success".to_string(),
                    backend: "sandbox".to_string(),
                    exit_code: 0,
                    policy_decision: "allow".to_string(),
                    runtime_allowed: true,
                    degrade_reason: None,
                    runtime_deny_reason: None,
                }),
                display_name: Some("Run Shell".to_string()),
            },
            Event::ToolCallArgsDelta {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                args_delta: "{}".to_string(),
                display_name: Some("Run Shell".to_string()),
            },
            Event::ToolCallReady {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                display_name: Some("Run Shell".to_string()),
                target: Some("cmd".to_string()),
            },
            Event::ToolCallProgress {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                progress_message: "running".to_string(),
                display_name: Some("Run Shell".to_string()),
            },
            Event::ToolCallFailed {
                turn_id: 1,
                tool_call_id: "call_1".to_string(),
                tool_name: "shell".to_string(),
                code: "TOOL_FAILED".to_string(),
                message: "failed".to_string(),
                retryable: false,
                display_name: Some("Run Shell".to_string()),
            },
            Event::AssistantMessage {
                turn_id: 1,
                content: "legacy".to_string(),
            },
            Event::ResponseStarted {
                turn_id: 1,
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
            Event::ResponseTextDelta {
                turn_id: 1,
                content_delta: "hello".to_string(),
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
            Event::ReasoningDelta {
                turn_id: 1,
                content_delta: "think".to_string(),
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
            Event::ToolCallsProposed {
                turn_id: 1,
                tool_calls: vec![ToolCallProposal {
                    tool_call_id: "call_2".to_string(),
                    tool_name: "search".to_string(),
                    arguments: BTreeMap::new(),
                    depends_on: vec![],
                    concurrency_group: None,
                }],
            },
            Event::ToolBatchCompleted {
                turn_id: 1,
                total: 1,
                succeeded: 1,
                failed: 0,
            },
            Event::ResponseResumed {
                turn_id: 1,
                stream_source: openjax_protocol::StreamSource::Replay,
            },
            Event::ResponseCompleted {
                turn_id: 1,
                content: "done".to_string(),
                stream_source: openjax_protocol::StreamSource::ModelLive,
            },
            Event::LoopWarning {
                turn_id: 1,
                tool_name: "shell".to_string(),
                consecutive_count: 3,
            },
            Event::ResponseError {
                turn_id: 1,
                code: "UPSTREAM".to_string(),
                message: "err".to_string(),
                retryable: false,
            },
            Event::ApprovalRequested {
                turn_id: 1,
                request_id: "appr_1".to_string(),
                target: "shell".to_string(),
                reason: "need approval".to_string(),
                policy_version: Some(1),
                matched_rule_id: Some("rule_1".to_string()),
                tool_name: Some("shell".to_string()),
                command_preview: Some("rm -rf /".to_string()),
                risk_tags: vec!["filesystem".to_string()],
                sandbox_backend: Some("sandbox-exec".to_string()),
                degrade_reason: None,
                approval_kind: Some(ApprovalKind::Normal),
            },
            Event::ApprovalResolved {
                turn_id: 1,
                request_id: "appr_1".to_string(),
                approved: true,
            },
            Event::ContextUsageUpdated {
                turn_id: 1,
                input_tokens: 128,
                context_window_size: 128000,
                ratio: 0.001,
            },
            Event::AgentSpawned {
                parent_thread_id: Some(ThreadId::new()),
                new_thread_id: ThreadId::new(),
            },
            Event::AgentStatusChanged {
                thread_id: ThreadId::new(),
                status: AgentStatus::Running,
            },
            Event::ContextCompacted {
                turn_id: 1,
                compressed_turns: 4,
                retained_turns: 2,
                summary_preview: "summary".to_string(),
            },
            Event::TurnCompleted { turn_id: 1 },
            Event::ShutdownComplete,
        ]
    }

    fn collect_unmapped_with<F>(events: &[Event], mapper: F) -> Vec<&'static str>
    where
        F: Fn(&Event) -> MapResult,
    {
        events
            .iter()
            .filter_map(|event| match mapper(event) {
                MapResult::Unmapped(name) => Some(name),
                MapResult::Mapped(_) | MapResult::IgnoredInternal => None,
            })
            .collect()
    }

    #[test]
    fn mapping_gate_fails_when_core_event_variant_not_covered() {
        let unmapped = collect_unmapped_with(&[Event::TurnStarted { turn_id: 1 }], |_| {
            MapResult::Unmapped("turn_started")
        });
        assert_eq!(unmapped, vec!["turn_started"]);
    }

    #[test]
    fn mapping_gate_covers_all_current_core_event_variants() {
        let unmapped = collect_unmapped_with(
            sample_events_covering_all_variants().as_slice(),
            map_core_event_payload_result,
        );
        assert!(
            unmapped.is_empty(),
            "unmapped core event variants: {unmapped:?}"
        );
    }

    #[test]
    fn maps_context_usage_updated_event() {
        let mapping = map_core_event_payload(&Event::ContextUsageUpdated {
            turn_id: 7,
            input_tokens: 4096,
            context_window_size: 128000,
            ratio: 0.032,
        })
        .expect("mapping should exist");

        assert_eq!(mapping.core_turn_id, Some(7));
        assert_eq!(mapping.event_type, "context_usage_updated");
        assert_eq!(
            mapping
                .payload
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            4096
        );
        assert_eq!(
            mapping
                .payload
                .get("context_window_size")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            128000
        );
        assert!(
            mapping
                .payload
                .get("ratio")
                .and_then(|v| v.as_f64())
                .unwrap_or_default()
                > 0.03
        );
        assert_eq!(mapping.stream_source, None);
    }

    #[test]
    fn maps_tool_call_completed_with_shell_metadata() {
        let mapping = map_core_event_payload(&Event::ToolCallCompleted {
            turn_id: 7,
            tool_call_id: "call_1".to_string(),
            tool_name: "shell".to_string(),
            ok: true,
            output: "done".to_string(),
            shell_metadata: Some(ShellExecutionMetadata {
                result_class: "success".to_string(),
                backend: "sandbox".to_string(),
                exit_code: 0,
                policy_decision: "allow".to_string(),
                runtime_allowed: true,
                degrade_reason: None,
                runtime_deny_reason: None,
            }),
            display_name: Some("Run Shell".to_string()),
        })
        .expect("mapping should exist");

        assert_eq!(mapping.core_turn_id, Some(7));
        assert_eq!(mapping.event_type, "tool_call_completed");
        assert_eq!(mapping.payload["tool_call_id"], "call_1");
        assert_eq!(mapping.payload["display_name"], "Run Shell");
        assert_eq!(mapping.payload["shell_metadata"]["backend"], "sandbox");
    }
}
