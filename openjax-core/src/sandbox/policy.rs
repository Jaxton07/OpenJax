use std::collections::BTreeSet;

use crate::tools::context::{ApprovalPolicy, SandboxPolicy, ToolInvocation, ToolPayload};

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
    invocation: &ToolInvocation,
    is_mutating: bool,
) -> PolicyOutcome {
    let mut trace = PolicyTrace {
        decision: PolicyDecision::Allow,
        reason: "allowed by default".to_string(),
        risk_tags: Vec::new(),
        capabilities: Vec::new(),
    };

    let mut approval_context: Option<ApprovalContext> = None;

    if is_shell_like_tool(&invocation.tool_name) {
        let shell = analyze_shell_invocation(invocation);
        trace = shell.trace.clone();
        approval_context = shell.approval_context;
    } else if is_mutating {
        trace.decision = PolicyDecision::AskApproval;
        trace.reason = "mutating tool requires approval".to_string();
    }

    match invocation.turn.approval_policy {
        ApprovalPolicy::AlwaysAsk => {
            if !matches!(trace.decision, PolicyDecision::Deny) {
                trace.decision = PolicyDecision::AskApproval;
                if trace.reason == "allowed by default" {
                    trace.reason = "approval policy requires confirmation".to_string();
                }
                if approval_context.is_none() {
                    approval_context = Some(ApprovalContext {
                        tool_name: invocation.tool_name.clone(),
                        raw_command: None,
                        normalized_command: None,
                        command_preview: None,
                        risk_tags: trace.risk_tags.clone(),
                        reason: trace.reason.clone(),
                        sandbox_backend: None,
                        degrade_reason: None,
                        fallback_plan: None,
                    });
                }
            }
        }
        ApprovalPolicy::OnRequest => {}
        ApprovalPolicy::Never => {
            if matches!(
                trace.decision,
                PolicyDecision::AskApproval | PolicyDecision::AskEscalation
            ) {
                if is_shell_like_tool(&invocation.tool_name) {
                    trace.decision = PolicyDecision::Deny;
                    trace.reason = "approval is required but policy is set to never".to_string();
                } else {
                    trace.decision = PolicyDecision::Allow;
                    trace.reason = "approval policy is never; mutating tool allowed".to_string();
                }
            }
        }
    }

    PolicyOutcome {
        trace,
        approval_context,
    }
}

fn analyze_shell_invocation(invocation: &ToolInvocation) -> PolicyOutcome {
    let Some((command, require_escalated)) = extract_shell_command(invocation) else {
        return PolicyOutcome {
            trace: PolicyTrace {
                decision: PolicyDecision::Deny,
                reason: "invalid shell payload".to_string(),
                risk_tags: vec!["invalid_payload".to_string()],
                capabilities: vec![SandboxCapability::ProcessExec],
            },
            approval_context: Some(ApprovalContext {
                tool_name: invocation.tool_name.clone(),
                raw_command: None,
                normalized_command: None,
                command_preview: None,
                risk_tags: vec!["invalid_payload".to_string()],
                reason: "invalid shell payload".to_string(),
                sandbox_backend: preferred_backend(invocation.turn.sandbox_policy),
                degrade_reason: None,
                fallback_plan: None,
            }),
        };
    };

    let normalized = normalize_command(&command);
    let mut risks = BTreeSet::new();
    let capabilities = detect_capabilities(&normalized);
    let decision = decide_shell_policy(
        invocation.turn.sandbox_policy,
        &normalized,
        require_escalated,
        &capabilities,
        &mut risks,
    );

    let risk_tags: Vec<String> = risks.into_iter().collect();
    let reason = match decision {
        PolicyDecision::Allow => "command allowed by policy".to_string(),
        PolicyDecision::AskApproval => "command requires approval".to_string(),
        PolicyDecision::AskEscalation => "command requires escalated approval".to_string(),
        PolicyDecision::Deny => "command denied by policy".to_string(),
    };
    let approval_context = if matches!(
        decision,
        PolicyDecision::AskApproval | PolicyDecision::AskEscalation
    ) {
        Some(ApprovalContext {
            tool_name: invocation.tool_name.clone(),
            raw_command: Some(command.clone()),
            normalized_command: Some(normalized.clone()),
            command_preview: Some(truncate_preview(&normalized, 180)),
            risk_tags: risk_tags.clone(),
            reason: reason.clone(),
            sandbox_backend: preferred_backend(invocation.turn.sandbox_policy),
            degrade_reason: None,
            fallback_plan: None,
        })
    } else {
        None
    };

    PolicyOutcome {
        trace: PolicyTrace {
            decision,
            reason,
            risk_tags,
            capabilities,
        },
        approval_context,
    }
}

fn decide_shell_policy(
    sandbox_policy: SandboxPolicy,
    command: &str,
    require_escalated: bool,
    capabilities: &[SandboxCapability],
    risks: &mut BTreeSet<String>,
) -> PolicyDecision {
    let lower = command.to_ascii_lowercase();
    let destructive_patterns = ["rm -rf /", "mkfs", "dd if=", ":(){:|:&};:"];
    if destructive_patterns.iter().any(|p| lower.contains(p)) {
        risks.insert("destructive".to_string());
        return PolicyDecision::Deny;
    }

    if require_escalated {
        risks.insert("require_escalated".to_string());
        return PolicyDecision::AskEscalation;
    }

    if matches!(sandbox_policy, SandboxPolicy::DangerFullAccess) {
        return PolicyDecision::Allow;
    }

    if lower.contains("sudo ") {
        risks.insert("privilege_escalation".to_string());
        return PolicyDecision::AskEscalation;
    }

    let has_network = capabilities.contains(&SandboxCapability::Network);
    let has_write = capabilities.contains(&SandboxCapability::FsWrite)
        || capabilities.contains(&SandboxCapability::EnvWrite);
    if has_network {
        risks.insert("network".to_string());
    }
    if has_write {
        risks.insert("write".to_string());
    }
    if has_network || has_write {
        return PolicyDecision::AskApproval;
    }

    PolicyDecision::Allow
}

fn detect_capabilities(command: &str) -> Vec<SandboxCapability> {
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

fn extract_shell_command(invocation: &ToolInvocation) -> Option<(String, bool)> {
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

fn normalize_command(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_preview(command: &str, limit: usize) -> String {
    let total = command.chars().count();
    if total <= limit {
        return command.to_string();
    }
    let mut preview = command.chars().take(limit).collect::<String>();
    preview.push_str("...");
    preview
}

fn preferred_backend(policy: SandboxPolicy) -> Option<SandboxBackend> {
    match policy {
        SandboxPolicy::DangerFullAccess => Some(SandboxBackend::NoneEscalated),
        SandboxPolicy::Write | SandboxPolicy::ReadOnly => {
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
        SandboxPolicy::None => Some(SandboxBackend::NoneEscalated),
    }
}

fn is_shell_like_tool(tool_name: &str) -> bool {
    matches!(tool_name, "shell" | "exec_command")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{PolicyDecision, SandboxCapability, evaluate_tool_invocation_policy};
    use crate::approval::StdinApprovalHandler;
    use crate::tools::context::{
        ApprovalPolicy, SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext,
    };

    fn shell_invocation(command: &str) -> ToolInvocation {
        ToolInvocation {
            tool_name: "shell".to_string(),
            call_id: "cid".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({ "cmd": command }).to_string(),
            },
            turn: ToolTurnContext {
                turn_id: 1,
                cwd: std::path::PathBuf::from("."),
                sandbox_policy: SandboxPolicy::Write,
                approval_policy: ApprovalPolicy::OnRequest,
                approval_handler: Arc::new(StdinApprovalHandler::new()),
                event_sink: None,
                windows_sandbox_level: None,
                prevent_shell_skill_trigger: true,
            },
        }
    }

    #[test]
    fn shell_pipe_is_not_auto_denied() {
        let invocation = shell_invocation("ps -eo pid,%cpu,cmd --sort=-%cpu | head -n 2");
        let outcome = evaluate_tool_invocation_policy(&invocation, true);
        assert_ne!(outcome.trace.decision, PolicyDecision::Deny);
    }

    #[test]
    fn shell_network_requires_approval() {
        let invocation = shell_invocation("curl https://example.com");
        let outcome = evaluate_tool_invocation_policy(&invocation, true);
        assert_eq!(outcome.trace.decision, PolicyDecision::AskApproval);
        assert!(
            outcome
                .trace
                .capabilities
                .contains(&SandboxCapability::Network)
        );
    }

    #[test]
    fn shell_git_commit_requires_approval() {
        let invocation = shell_invocation("git commit -m \"feat: test\"");
        let outcome = evaluate_tool_invocation_policy(&invocation, true);
        assert_eq!(outcome.trace.decision, PolicyDecision::AskApproval);
        assert!(
            outcome
                .trace
                .capabilities
                .contains(&SandboxCapability::FsWrite)
        );
    }
}
