use std::collections::HashMap;

use super::shell::ShellType;

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

pub fn parse_tool_call(input: &str) -> Option<ToolCall> {
    let input = input.trim();
    
    if !input.starts_with("tool:") {
        return None;
    }
    
    let rest = &input[5..].trim();
    if rest.is_empty() {
        return None;
    }
    
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    
    let name = parts[0].to_string();
    let mut args = HashMap::new();
    
    for part in parts.iter().skip(1) {
        if let Some((key, value)) = part.split_once('=') {
            args.insert(key.to_string(), value.to_string());
        }
    }
    
    Some(ToolCall { name, args })
}

#[derive(Debug, Clone, Copy)]
pub struct ToolRuntimeConfig {
    pub approval_policy: ApprovalPolicy,
    pub sandbox_mode: SandboxMode,
    pub shell_type: ShellType,
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::AlwaysAsk,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    AlwaysAsk,
    OnRequest,
    Never,
}

impl ApprovalPolicy {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_APPROVAL_POLICY").as_deref() {
            Ok("always_ask") => Self::AlwaysAsk,
            Ok("on_request") => Self::OnRequest,
            Ok("never") => Self::Never,
            _ => Self::AlwaysAsk,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlwaysAsk => "always_ask",
            Self::OnRequest => "on_request",
            Self::Never => "never",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_MODE").as_deref() {
            Ok("workspace_write") => Self::WorkspaceWrite,
            Ok("danger_full_access") => Self::DangerFullAccess,
            _ => Self::WorkspaceWrite,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkspaceWrite => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }
}

pub const MAX_AGENT_DEPTH: i32 = 10;
