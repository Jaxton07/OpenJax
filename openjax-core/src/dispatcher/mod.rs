mod errors;
mod metrics;
mod probe;
mod route_tool;
mod state_machine;

use std::time::Duration;

pub(crate) use errors::DispatchError;
pub(crate) use metrics::{DispatchMetrics, DispatchTiming};
pub(crate) use probe::{ProbeInput, probe_branch};
pub(crate) use route_tool::emit_tool_call_ready;
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
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DispatcherConfig {
    pub(crate) probe_window: Duration,
    pub(crate) heuristic_detect: bool,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            probe_window: Duration::from_millis(80),
            heuristic_detect: false,
        }
    }
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
    tracing::debug!(
        turn_id = input.turn_id,
        probe_window_ms = cfg.probe_window.as_millis() as u64,
        "dispatch_probe_started"
    );

    let probe = probe_branch(input, cfg);
    let locked_branch = probe.branch;
    machine.lock(locked_branch);
    let _ = machine.complete();
    let mut metrics = DispatchMetrics::default();
    metrics.dispatcher_lock_ts_ms = Some(timing.elapsed_ms());
    let meta = DispatchDecisionMeta {
        probe_ms: metrics.dispatcher_lock_ts_ms.unwrap_or_default(),
        locked_branch,
        signal_source: probe.signal_source,
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
