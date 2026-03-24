use std::path::PathBuf;
use std::sync::Arc;

use crate::approval::{ApprovalHandler, StdinApprovalHandler};
use crate::tools::shell::ShellType;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::{
    DecisionKind as PolicyCenterDecisionKind, PolicyInput as PolicyCenterInput,
    PolicyRule as PolicyCenterRule,
};
use openjax_protocol::Event;
use tokio::sync::mpsc::UnboundedSender;

/// 工具调用载荷
#[derive(Debug, Clone)]
pub enum ToolPayload {
    Function {
        arguments: String,
    },
    Custom {
        input: String,
    },
    LocalShell {
        params: ShellToolCallParams,
    },
    Mcp {
        server: String,
        tool: String,
        raw_arguments: String,
    },
}

/// Shell 工具调用参数
#[derive(Debug, Clone)]
pub struct ShellToolCallParams {
    pub command: String,
    pub args: Vec<String>,
}

/// 工具策略描述符
#[derive(Debug, Clone)]
pub struct PolicyDescriptor {
    pub action: String,
    pub capabilities: Vec<String>,
    pub risk_tags: Vec<String>,
}

impl PolicyDescriptor {
    pub fn allow_rule_for_tool(&self, tool_name: &str) -> PolicyCenterRule {
        PolicyCenterRule {
            id: format!("descriptor:{tool_name}:{}", self.action),
            decision: PolicyCenterDecisionKind::Allow,
            priority: 100,
            tool_name: Some(tool_name.to_string()),
            action: Some(self.action.clone()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: self.capabilities.clone(),
            risk_tags_all: self.risk_tags.clone(),
            reason: format!("tool `{tool_name}` is declared in policy descriptor"),
        }
    }
}

/// 工具输出
#[derive(Debug, Clone)]
pub enum ToolOutput {
    Function {
        body: FunctionCallOutputBody,
        success: Option<bool>,
    },
    Mcp {
        result: Result<McpToolResult, String>,
    },
}

/// 函数调用输出体
#[derive(Debug, Clone)]
pub enum FunctionCallOutputBody {
    Text(String),
    Json(serde_json::Value),
}

/// 工具调用上下文
#[derive(Clone)]
pub struct ToolInvocation {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
    pub turn: ToolTurnContext,
}

impl std::fmt::Debug for ToolInvocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolInvocation")
            .field("tool_name", &self.tool_name)
            .field("call_id", &self.call_id)
            .field("payload", &self.payload)
            .field("turn", &self.turn)
            .finish()
    }
}

impl ToolInvocation {
    pub fn policy_descriptor(&self) -> Option<PolicyDescriptor> {
        let descriptor = match self.tool_name.as_str() {
            "read_file" | "list_dir" | "grep_files" => PolicyDescriptor {
                action: "read".to_string(),
                capabilities: vec!["fs_read".to_string()],
                risk_tags: vec![],
            },
            "apply_patch" | "edit_file_range" => PolicyDescriptor {
                action: "write".to_string(),
                capabilities: vec!["fs_write".to_string()],
                risk_tags: vec!["mutating".to_string()],
            },
            "process_snapshot" | "system_load" | "disk_usage" => PolicyDescriptor {
                action: "observe".to_string(),
                capabilities: vec!["process_exec".to_string()],
                risk_tags: vec![],
            },
            "shell" | "exec_command" => {
                let mut risk_tags = Vec::new();
                if shell_payload_requires_escalated(&self.payload) {
                    risk_tags.push("require_escalated".to_string());
                }
                PolicyDescriptor {
                    action: "exec".to_string(),
                    capabilities: vec!["process_exec".to_string()],
                    risk_tags,
                }
            }
            _ => return None,
        };
        Some(descriptor)
    }

    pub fn to_policy_center_input(
        &self,
        descriptor: Option<&PolicyDescriptor>,
        policy_version: u64,
    ) -> PolicyCenterInput {
        let (action, capabilities, risk_tags) = descriptor
            .map(|d| {
                (
                    d.action.clone(),
                    d.capabilities.clone(),
                    d.risk_tags.clone(),
                )
            })
            .unwrap_or_else(|| {
                (
                    "invoke".to_string(),
                    Vec::new(),
                    vec!["unknown_tool_descriptor".to_string()],
                )
            });

        PolicyCenterInput {
            tool_name: self.tool_name.clone(),
            action,
            session_id: self
                .turn
                .session_id
                .clone()
                .or_else(|| Some(self.turn.turn_id.to_string())),
            actor: Some("user".to_string()),
            resource: Some(self.turn.cwd.display().to_string()),
            capabilities,
            risk_tags,
            policy_version,
        }
    }
}

fn shell_payload_requires_escalated(payload: &ToolPayload) -> bool {
    let ToolPayload::Function { arguments } = payload else {
        return false;
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return false;
    };

    match value.get("require_escalated") {
        Some(serde_json::Value::Bool(v)) => *v,
        Some(serde_json::Value::String(v)) => {
            matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
        }
        _ => false,
    }
}

/// 工具轮次上下文
#[derive(Clone)]
pub struct ToolTurnContext {
    pub turn_id: u64,
    pub session_id: Option<String>,
    pub cwd: PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub shell_type: ShellType,
    pub approval_handler: Arc<dyn ApprovalHandler>,
    pub event_sink: Option<UnboundedSender<Event>>,
    pub policy_runtime: Option<PolicyRuntime>,
    pub windows_sandbox_level: Option<String>,
    pub prevent_shell_skill_trigger: bool,
}

impl Default for ToolTurnContext {
    fn default() -> Self {
        Self {
            turn_id: 0,
            session_id: None,
            cwd: PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(StdinApprovalHandler::new()),
            event_sink: None,
            policy_runtime: None,
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        }
    }
}

impl std::fmt::Debug for ToolTurnContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolTurnContext")
            .field("turn_id", &self.turn_id)
            .field("cwd", &self.cwd)
            .field("sandbox_policy", &self.sandbox_policy)
            .field("shell_type", &self.shell_type)
            .field("session_id", &self.session_id)
            .field(
                "policy_runtime",
                &self.policy_runtime.as_ref().map(|_| "<runtime>"),
            )
            .field("windows_sandbox_level", &self.windows_sandbox_level)
            .field(
                "prevent_shell_skill_trigger",
                &self.prevent_shell_skill_trigger,
            )
            .finish()
    }
}


/// MCP 工具结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpToolResult {
    pub content: Option<serde_json::Value>,
    pub is_error: Option<bool>,
}

/// 沙箱策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
    None,
    ReadOnly,
    Write,
    DangerFullAccess,
}

impl SandboxPolicy {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_MODE").as_deref() {
            Ok("danger_full_access") => Self::DangerFullAccess,
            Ok("workspace_write") => Self::Write,
            Ok("read_only") => Self::ReadOnly,
            _ => Self::Write,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read_only",
            Self::Write => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }
}
