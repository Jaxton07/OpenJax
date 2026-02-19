use std::collections::HashMap;

use super::shell::ShellType;
use super::spec::ToolsConfig;
pub use super::context::ApprovalPolicy;

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
    
    let parts: Vec<String> = shlex::split(rest).unwrap_or_else(|| {
        rest.split_whitespace().map(ToString::to_string).collect()
    });
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
    pub tools_config: ToolsConfig,
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
            tools_config: ToolsConfig::default(),
        }
    }
}

impl ToolRuntimeConfig {
    pub fn with_config(config: ToolsConfig) -> Self {
        Self {
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
            tools_config: config,
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

#[cfg(test)]
mod tests {
    use super::parse_tool_call;

    #[test]
    fn parse_tool_call_preserves_quoted_shell_command() {
        let input = "tool:shell cmd='echo hi >/tmp/openjax-e2e.txt' require_escalated=true";
        let parsed = parse_tool_call(input).expect("expected parsed tool call");
        assert_eq!(parsed.name, "shell");
        assert_eq!(
            parsed.args.get("cmd").map(String::as_str),
            Some("echo hi >/tmp/openjax-e2e.txt")
        );
        assert_eq!(
            parsed.args.get("require_escalated").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn parse_tool_call_preserves_quoted_apply_patch() {
        let input = "tool:apply_patch patch='*** Begin Patch\\n*** Add File: test.txt\\n+hello\\n*** End Patch'";
        let parsed = parse_tool_call(input).expect("expected parsed tool call");
        assert_eq!(parsed.name, "apply_patch");
        assert_eq!(
            parsed.args.get("patch").map(String::as_str),
            Some("*** Begin Patch\\n*** Add File: test.txt\\n+hello\\n*** End Patch")
        );
    }
}
