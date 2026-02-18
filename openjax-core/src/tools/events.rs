use serde::Serialize;

/// 工具使用前钩子
#[derive(Debug, Clone, Serialize)]
pub struct BeforeToolUse {
    pub tool_name: String,
    pub call_id: String,
    pub tool_input: String,
}

/// 工具使用后钩子
#[derive(Debug, Clone, Serialize)]
pub struct AfterToolUse {
    pub tool_name: String,
    pub call_id: String,
    pub tool_input: String,
    pub executed: bool,
    pub success: bool,
    pub duration_ms: u64,
    pub mutating: bool,
    pub sandbox: String,
    pub sandbox_policy: String,
    pub output_preview: Option<String>,
}

/// 钩子事件
#[derive(Debug, Clone)]
pub enum HookEvent {
    BeforeToolUse(BeforeToolUse),
    AfterToolUse(AfterToolUse),
}

impl HookEvent {
    pub fn tool_name(&self) -> &str {
        match self {
            HookEvent::BeforeToolUse(data) => &data.tool_name,
            HookEvent::AfterToolUse(data) => &data.tool_name,
        }
    }

    pub fn call_id(&self) -> &str {
        match self {
            HookEvent::BeforeToolUse(data) => &data.call_id,
            HookEvent::AfterToolUse(data) => &data.call_id,
        }
    }
}
