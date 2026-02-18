use std::path::PathBuf;

/// 工具调用载荷
#[derive(Debug, Clone)]
pub enum ToolPayload {
    Function { arguments: String },
    Custom { input: String },
    LocalShell { params: ShellToolCallParams },
    Mcp { server: String, tool: String, raw_arguments: String },
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
    Function { body: FunctionCallOutputBody, success: Option<bool> },
    Mcp { result: Result<McpToolResult, String> },
}

/// 函数调用输出体
#[derive(Debug, Clone)]
pub enum FunctionCallOutputBody {
    Text(String),
    Json(serde_json::Value),
}

/// 工具调用上下文
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
    pub turn: ToolTurnContext,
}

/// 工具轮次上下文
#[derive(Debug, Clone)]
pub struct ToolTurnContext {
    pub cwd: PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub windows_sandbox_level: Option<String>,
}

impl Default for ToolTurnContext {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            windows_sandbox_level: None,
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
