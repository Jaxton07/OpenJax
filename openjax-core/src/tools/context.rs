use std::path::PathBuf;
use std::sync::Arc;

use crate::approval::{ApprovalHandler, StdinApprovalHandler};
use crate::tools::shell::ShellType;
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

/// 工具轮次上下文
#[derive(Clone)]
pub struct ToolTurnContext {
    pub turn_id: u64,
    pub cwd: PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub approval_policy: ApprovalPolicy,
    pub shell_type: ShellType,
    pub approval_handler: Arc<dyn ApprovalHandler>,
    pub event_sink: Option<UnboundedSender<Event>>,
    pub windows_sandbox_level: Option<String>,
    pub prevent_shell_skill_trigger: bool,
}

impl Default for ToolTurnContext {
    fn default() -> Self {
        Self {
            turn_id: 0,
            cwd: PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            approval_policy: ApprovalPolicy::OnRequest,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(StdinApprovalHandler::new()),
            event_sink: None,
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
            .field("approval_policy", &self.approval_policy)
            .field("shell_type", &self.shell_type)
            .field("windows_sandbox_level", &self.windows_sandbox_level)
            .field(
                "prevent_shell_skill_trigger",
                &self.prevent_shell_skill_trigger,
            )
            .finish()
    }
}

/// 批准策略
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
            _ => Self::OnRequest,
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
