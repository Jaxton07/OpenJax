use std::sync::Arc;

/// 斜杠命令执行结果
#[derive(Clone)]
pub enum SlashResult {
    Ok(String),
    Err(String),
    Pending,
}

impl SlashResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, SlashResult::Ok(_))
    }
    pub fn message(&self) -> &str {
        match self {
            SlashResult::Ok(s) => s.as_str(),
            SlashResult::Err(s) => s.as_str(),
            SlashResult::Pending => "",
        }
    }
    pub fn ok(self) -> Option<String> {
        match self {
            SlashResult::Ok(s) => Some(s),
            _ => None,
        }
    }
}

/// 命令执行类型
#[derive(Clone)]
pub enum SlashCommandKind {
    Builtin {
        handler: Arc<dyn Fn() -> (String, bool) + Send + Sync>,
        /// 执行后是否用返回文本替换输入框内容
        replaces_input: bool,
    },
    SessionAction {
        action: &'static str,
    },
    Skill {
        skill_name: &'static str,
    },
}

impl SlashCommandKind {
    pub fn execute(&self) -> SlashResult {
        match self {
            SlashCommandKind::Builtin { handler, .. } => SlashResult::Ok(handler().0),
            SlashCommandKind::SessionAction { .. } | SlashCommandKind::Skill { .. } => {
                SlashResult::Pending
            }
        }
    }
    pub fn needs_agent(&self) -> bool {
        matches!(
            self,
            SlashCommandKind::SessionAction { .. } | SlashCommandKind::Skill { .. }
        )
    }
    pub fn session_action_name(&self) -> Option<&'static str> {
        match self {
            SlashCommandKind::SessionAction { action } => Some(action),
            _ => None,
        }
    }
    /// 仅对 Builtin 类型有效，返回是否替换输入框
    pub fn replaces_input(&self) -> bool {
        match self {
            SlashCommandKind::Builtin { replaces_input, .. } => *replaces_input,
            _ => false,
        }
    }
}
