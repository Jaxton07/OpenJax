use crate::{Config, tools};

/// 单回合最大工具调用次数，也是单回合最大规划轮次。
/// 同时作为 LoopDetector 滑动窗口的基础容量。
pub const MAX_TURN_BUDGET: usize = 300;

const DEFAULT_MAX_TOOL_CALLS_PER_TURN: usize = MAX_TURN_BUDGET;
const DEFAULT_MAX_PLANNER_ROUNDS_PER_TURN: usize = MAX_TURN_BUDGET;

pub(crate) fn parse_approval_policy(value: &str) -> Option<tools::ApprovalPolicy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "always_ask" => Some(tools::ApprovalPolicy::AlwaysAsk),
        "on_request" => Some(tools::ApprovalPolicy::OnRequest),
        "never" => Some(tools::ApprovalPolicy::Never),
        _ => None,
    }
}

pub(crate) fn parse_sandbox_mode(value: &str) -> Option<tools::SandboxMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "workspace_write" => Some(tools::SandboxMode::WorkspaceWrite),
        "danger_full_access" => Some(tools::SandboxMode::DangerFullAccess),
        _ => None,
    }
}

pub(crate) fn resolve_approval_policy(config: &Config) -> tools::ApprovalPolicy {
    if let Ok(val) = std::env::var("OPENJAX_APPROVAL_POLICY")
        && let Some(policy) = parse_approval_policy(&val)
    {
        return policy;
    }

    if let Some(policy) = config
        .sandbox
        .as_ref()
        .and_then(|s| s.approval_policy.as_deref())
        .and_then(parse_approval_policy)
    {
        return policy;
    }

    tools::ApprovalPolicy::OnRequest
}

pub(crate) fn resolve_sandbox_mode(config: &Config) -> tools::SandboxMode {
    if let Ok(val) = std::env::var("OPENJAX_SANDBOX_MODE")
        && let Some(mode) = parse_sandbox_mode(&val)
    {
        return mode;
    }

    if let Some(mode) = config
        .sandbox
        .as_ref()
        .and_then(|s| s.mode.as_deref())
        .and_then(parse_sandbox_mode)
    {
        return mode;
    }

    tools::SandboxMode::WorkspaceWrite
}

pub(crate) fn resolve_max_tool_calls_per_turn(config: &Config) -> usize {
    resolve_max_tool_calls_per_turn_with_lookup(config, |key| std::env::var(key).ok())
}

pub(crate) fn resolve_max_planner_rounds_per_turn(config: &Config) -> usize {
    resolve_max_planner_rounds_per_turn_with_lookup(config, |key| std::env::var(key).ok())
}

pub(crate) fn resolve_max_tool_calls_per_turn_with_lookup<F>(config: &Config, lookup: F) -> usize
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(raw) = lookup("OPENJAX_MAX_TOOL_CALLS_PER_TURN")
        && let Some(parsed) = parse_positive_usize(&raw)
    {
        return parsed;
    }

    if let Some(parsed) = config
        .agent
        .as_ref()
        .and_then(|agent| agent.max_tool_calls_per_turn)
        .filter(|value| *value > 0)
    {
        return parsed;
    }

    DEFAULT_MAX_TOOL_CALLS_PER_TURN
}

pub(crate) fn resolve_max_planner_rounds_per_turn_with_lookup<F>(
    config: &Config,
    lookup: F,
) -> usize
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(raw) = lookup("OPENJAX_MAX_PLANNER_ROUNDS_PER_TURN")
        && let Some(parsed) = parse_positive_usize(&raw)
    {
        return parsed;
    }

    if let Some(parsed) = config
        .agent
        .as_ref()
        .and_then(|agent| agent.max_planner_rounds_per_turn)
        .filter(|value| *value > 0)
    {
        return parsed;
    }

    DEFAULT_MAX_PLANNER_ROUNDS_PER_TURN
}

fn parse_positive_usize(value: &str) -> Option<usize> {
    value.trim().parse::<usize>().ok().filter(|v| *v > 0)
}
