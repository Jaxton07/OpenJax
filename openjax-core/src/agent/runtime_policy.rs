use crate::{Config, tools};

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
    if let Ok(val) = std::env::var("OPENJAX_APPROVAL_POLICY") {
        if let Some(policy) = parse_approval_policy(&val) {
            return policy;
        }
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
    if let Ok(val) = std::env::var("OPENJAX_SANDBOX_MODE") {
        if let Some(mode) = parse_sandbox_mode(&val) {
            return mode;
        }
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
