// Legacy dispatcher path for historical JSON planner outputs. Kept as a
// non-primary helper while native tool calling remains the runtime default.
mod errors;
mod metrics;
mod probe;
mod state_machine;

use crate::agent::decision::{
    ModelDecision, NormalizedToolCall, normalize_model_decision, normalize_tool_calls,
    parse_model_decision, parse_model_decision_v2,
};
use crate::logger::AFTER_DISPATCH_LOG_TARGET;

const FLOW_TRACE_PREFIX: &str = "OPENJAX_FLOW";
const AFTER_DISPATCH_PREFIX: &str = "OPENJAX_AFTER_DISPATCH";

pub(crate) use errors::DispatchError;
pub(crate) use metrics::{DispatchMetrics, DispatchTiming};
pub(crate) use probe::{ProbeInput, probe_branch};
pub(crate) use state_machine::DispatchStateMachine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DispatchBranch {
    Text,
    ToolCall,
}

impl DispatchBranch {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::ToolCall => "tool_call",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DispatchSignalSource {
    ProviderStructured,
    AdapterHint,
    Heuristic,
    Default,
}

impl DispatchSignalSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProviderStructured => "provider_structured",
            Self::AdapterHint => "adapter_hint",
            Self::Heuristic => "heuristic",
            Self::Default => "default",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DispatchDecisionMeta {
    pub(crate) probe_ms: u64,
    pub(crate) locked_branch: DispatchBranch,
    pub(crate) signal_source: DispatchSignalSource,
    pub(crate) conflict_detected: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DispatcherConfig {
    pub(crate) heuristic_detect: bool,
}

impl DispatcherConfig {
    pub(crate) fn from_env() -> Self {
        // Keep dispatcher behavior deterministic by default and avoid extra compatibility toggles.
        Self::default()
    }
}

pub(crate) fn dispatch_stream(input: ProbeInput, cfg: DispatcherConfig) -> DispatchDecisionMeta {
    let mut machine = DispatchStateMachine::new();
    let timing = DispatchTiming::started();
    machine.enter_probing();
    tracing::debug!(turn_id = input.turn_id, "dispatch_probe_started");

    let probe = probe_branch(input, cfg);
    let locked_branch = probe.branch;
    machine.lock(locked_branch);
    let _ = machine.complete();
    let metrics = DispatchMetrics {
        dispatcher_lock_ts_ms: Some(timing.elapsed_ms()),
        ..Default::default()
    };
    let meta = DispatchDecisionMeta {
        probe_ms: metrics.dispatcher_lock_ts_ms.unwrap_or_default(),
        locked_branch,
        signal_source: probe.signal_source,
        conflict_detected: false,
    };

    tracing::debug!(
        turn_id = probe.turn_id,
        locked_branch = meta.locked_branch.as_str(),
        signal_source = meta.signal_source.as_str(),
        probe_ms = meta.probe_ms,
        "dispatch_branch_locked"
    );

    meta
}

#[derive(Debug)]
pub(crate) enum DispatchOutcome {
    ToolBatch {
        meta: DispatchDecisionMeta,
        calls: Vec<NormalizedToolCall>,
    },
    Tool {
        meta: DispatchDecisionMeta,
        decision: ModelDecision,
    },
    Final {
        meta: DispatchDecisionMeta,
        decision: ModelDecision,
    },
    Repair {
        meta: DispatchDecisionMeta,
        raw_output: String,
        reason: &'static str,
    },
    Error {
        code: &'static str,
        message: String,
    },
}

fn log_after_dispatch(
    turn_id: u64,
    route: &'static str,
    next: &'static str,
    meta: Option<DispatchDecisionMeta>,
    code: Option<&'static str>,
    reason: Option<&'static str>,
) {
    tracing::info!(
        target: AFTER_DISPATCH_LOG_TARGET,
        turn_id = turn_id,
        flow_prefix = AFTER_DISPATCH_PREFIX,
        flow_node = "dispatcher.output",
        flow_route = route,
        flow_next = next,
        conflict_detected = meta.map(|m| m.conflict_detected),
        signal_source = meta.map(|m| m.signal_source.as_str()),
        locked_branch = meta.map(|m| m.locked_branch.as_str()),
        flow_code = code,
        flow_reason = reason,
        "after_dispatcher_trace"
    );
}

pub(crate) fn route_model_output(
    input: ProbeInput<'_>,
    model_output: &str,
    tool_batch_v2_enabled: bool,
    cfg: DispatcherConfig,
) -> DispatchOutcome {
    let mut meta = dispatch_stream(input, cfg);
    let parsed_v2 = if tool_batch_v2_enabled {
        parse_model_decision_v2(model_output)
    } else {
        None
    };
    let parsed_v1 = parse_model_decision(model_output).map(normalize_model_decision);

    if let Some(v2) = parsed_v2 {
        let action = v2.action.to_ascii_lowercase();
        if action == "tool_batch" {
            let calls = normalize_tool_calls(&v2.tool_calls);
            if !calls.is_empty() {
                if meta.locked_branch != DispatchBranch::ToolCall {
                    meta.conflict_detected = true;
                }
                if meta.conflict_detected {
                    tracing::warn!(
                        turn_id = input.turn_id,
                        locked_branch = meta.locked_branch.as_str(),
                        selected_action = "tool_batch",
                        signal_source = meta.signal_source.as_str(),
                        "dispatch_conflict_resolved"
                    );
                }
                tracing::info!(
                    turn_id = input.turn_id,
                    flow_prefix = FLOW_TRACE_PREFIX,
                    flow_node = "dispatcher.route",
                    flow_route = "tool_batch",
                    flow_next = "planner.tool_batch",
                    conflict_detected = meta.conflict_detected,
                    signal_source = meta.signal_source.as_str(),
                    "flow_trace"
                );
                log_after_dispatch(
                    input.turn_id,
                    "tool_batch",
                    "planner.tool_batch",
                    Some(meta),
                    None,
                    None,
                );
                return DispatchOutcome::ToolBatch { meta, calls };
            }
            tracing::info!(
                turn_id = input.turn_id,
                flow_prefix = FLOW_TRACE_PREFIX,
                flow_node = "dispatcher.route",
                flow_route = "error",
                flow_next = "planner.error",
                flow_code = "model_invalid_tool_batch",
                "flow_trace"
            );
            log_after_dispatch(
                input.turn_id,
                "error",
                "planner.error",
                Some(meta),
                Some("model_invalid_tool_batch"),
                None,
            );
            return DispatchOutcome::Error {
                code: "model_invalid_tool_batch",
                message: "[model error] tool_batch missing valid tool_calls".to_string(),
            };
        }
    }

    if let Some(decision) = parsed_v1 {
        let action = decision.action.to_ascii_lowercase();
        if action == "tool" {
            if decision
                .tool
                .as_ref()
                .is_none_or(|name| name.trim().is_empty())
            {
                log_after_dispatch(
                    input.turn_id,
                    "error",
                    "planner.error",
                    Some(meta),
                    Some("model_invalid_tool"),
                    None,
                );
                return DispatchOutcome::Error {
                    code: "model_invalid_tool",
                    message: "[model error] tool action missing tool name".to_string(),
                };
            }
            if meta.locked_branch != DispatchBranch::ToolCall {
                meta.conflict_detected = true;
            }
            if meta.conflict_detected {
                tracing::warn!(
                    turn_id = input.turn_id,
                    locked_branch = meta.locked_branch.as_str(),
                    selected_action = "tool",
                    signal_source = meta.signal_source.as_str(),
                    "dispatch_conflict_resolved"
                );
            }
            tracing::info!(
                turn_id = input.turn_id,
                flow_prefix = FLOW_TRACE_PREFIX,
                flow_node = "dispatcher.route",
                flow_route = "tool",
                flow_next = "planner.tool_action",
                conflict_detected = meta.conflict_detected,
                signal_source = meta.signal_source.as_str(),
                "flow_trace"
            );
            log_after_dispatch(
                input.turn_id,
                "tool",
                "planner.tool_action",
                Some(meta),
                None,
                None,
            );
            return DispatchOutcome::Tool { meta, decision };
        }
        if action == "final" {
            if meta.locked_branch != DispatchBranch::Text {
                meta.conflict_detected = true;
            }
            if meta.conflict_detected {
                tracing::warn!(
                    turn_id = input.turn_id,
                    locked_branch = meta.locked_branch.as_str(),
                    selected_action = "final",
                    signal_source = meta.signal_source.as_str(),
                    "dispatch_conflict_resolved"
                );
            }
            tracing::info!(
                turn_id = input.turn_id,
                flow_prefix = FLOW_TRACE_PREFIX,
                flow_node = "dispatcher.route",
                flow_route = "final",
                flow_next = "frontend.response_stream",
                conflict_detected = meta.conflict_detected,
                signal_source = meta.signal_source.as_str(),
                "flow_trace"
            );
            log_after_dispatch(
                input.turn_id,
                "final",
                "frontend.response_stream",
                Some(meta),
                None,
                None,
            );
            return DispatchOutcome::Final { meta, decision };
        }
        tracing::info!(
            turn_id = input.turn_id,
            flow_prefix = FLOW_TRACE_PREFIX,
            flow_node = "dispatcher.route",
            flow_route = "error",
            flow_next = "planner.error",
            flow_code = "model_unsupported_action",
            action = %decision.action,
            "flow_trace"
        );
        log_after_dispatch(
            input.turn_id,
            "error",
            "planner.error",
            Some(meta),
            Some("model_unsupported_action"),
            None,
        );
        return DispatchOutcome::Error {
            code: "model_unsupported_action",
            message: format!("[model error] unsupported action: {}", decision.action),
        };
    }

    tracing::info!(
        turn_id = input.turn_id,
        flow_prefix = FLOW_TRACE_PREFIX,
        flow_node = "dispatcher.route",
        flow_route = "repair",
        flow_next = "planner.repair",
        flow_reason = "decision_parse_failed",
        "flow_trace"
    );
    log_after_dispatch(
        input.turn_id,
        "repair",
        "planner.repair",
        Some(meta),
        None,
        Some("decision_parse_failed"),
    );
    DispatchOutcome::Repair {
        meta,
        raw_output: model_output.to_string(),
        reason: "decision_parse_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_model_output_prefers_tool_batch_when_valid() {
        let output = r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"c1","tool_name":"list_dir","arguments":{"path":"."}}]}"#;
        let routed = route_model_output(
            ProbeInput {
                turn_id: 1,
                action_hint: Some("final"),
                model_output: Some(output),
            },
            output,
            true,
            DispatcherConfig::default(),
        );
        match routed {
            DispatchOutcome::ToolBatch { meta, calls } => {
                assert_eq!(calls.len(), 1);
                assert!(!meta.conflict_detected);
            }
            _ => panic!("expected tool_batch outcome"),
        }
    }

    #[test]
    fn route_model_output_returns_final_for_valid_final_action() {
        let output = r#"{"action":"final","message":"ok"}"#;
        let routed = route_model_output(
            ProbeInput {
                turn_id: 2,
                action_hint: Some("tool"),
                model_output: Some(output),
            },
            output,
            true,
            DispatcherConfig::default(),
        );
        assert!(matches!(routed, DispatchOutcome::Final { .. }));
    }

    #[test]
    fn route_model_output_returns_repair_on_unparseable_output() {
        let output = "not-json";
        let routed = route_model_output(
            ProbeInput {
                turn_id: 3,
                action_hint: None,
                model_output: Some(output),
            },
            output,
            true,
            DispatcherConfig::default(),
        );
        assert!(matches!(routed, DispatchOutcome::Repair { .. }));
    }
}
