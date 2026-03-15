use super::{DispatchBranch, DispatchSignalSource, DispatcherConfig};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProbeInput<'a> {
    pub(crate) turn_id: u64,
    pub(crate) action_hint: Option<&'a str>,
    pub(crate) model_output: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProbeResult {
    pub(crate) turn_id: u64,
    pub(crate) branch: DispatchBranch,
    pub(crate) signal_source: DispatchSignalSource,
}

pub(crate) fn probe_branch(input: ProbeInput<'_>, cfg: DispatcherConfig) -> ProbeResult {
    if matches!(input.action_hint, Some("tool" | "tool_batch")) {
        return ProbeResult {
            turn_id: input.turn_id,
            branch: DispatchBranch::ToolCall,
            signal_source: DispatchSignalSource::AdapterHint,
        };
    }

    if let Some(raw) = input.model_output {
        if raw.contains("\"tool_calls\"") {
            return ProbeResult {
                turn_id: input.turn_id,
                branch: DispatchBranch::ToolCall,
                signal_source: DispatchSignalSource::ProviderStructured,
            };
        }
        if cfg.heuristic_detect && raw.contains("\"action\":\"tool\"") {
            return ProbeResult {
                turn_id: input.turn_id,
                branch: DispatchBranch::ToolCall,
                signal_source: DispatchSignalSource::Heuristic,
            };
        }
    }

    ProbeResult {
        turn_id: input.turn_id,
        branch: DispatchBranch::Text,
        signal_source: DispatchSignalSource::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_hint_prefers_tool_call_branch() {
        let result = probe_branch(
            ProbeInput {
                turn_id: 1,
                action_hint: Some("tool"),
                model_output: Some("{\"action\":\"final\"}"),
            },
            DispatcherConfig::default(),
        );
        assert_eq!(result.branch, DispatchBranch::ToolCall);
        assert_eq!(result.signal_source, DispatchSignalSource::AdapterHint);
    }

    #[test]
    fn defaults_to_text_branch_without_tool_signal() {
        let result = probe_branch(
            ProbeInput {
                turn_id: 2,
                action_hint: None,
                model_output: Some("{\"action\":\"final\",\"message\":\"ok\"}"),
            },
            DispatcherConfig::default(),
        );
        assert_eq!(result.branch, DispatchBranch::Text);
        assert_eq!(result.signal_source, DispatchSignalSource::Default);
    }
}
