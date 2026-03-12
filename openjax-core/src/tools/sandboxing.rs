use crate::tools::context::SandboxPolicy;

/// 沙箱策略管理器
pub struct SandboxManager;

impl SandboxManager {
    pub fn new() -> Self {
        Self
    }

    /// 检查是否需要批准
    pub fn requires_approval(
        &self,
        sandbox_policy: SandboxPolicy,
        require_escalated: bool,
    ) -> bool {
        match sandbox_policy {
            SandboxPolicy::None => false,
            SandboxPolicy::ReadOnly => false,
            SandboxPolicy::Write => require_escalated,
            SandboxPolicy::DangerFullAccess => false,
        }
    }

    /// 选择合适的沙箱
    pub fn select_sandbox(&self, sandbox_policy: SandboxPolicy) -> SandboxPolicy {
        sandbox_policy
    }

    /// 检查是否为变异操作
    pub fn is_mutating_operation(&self, tool_name: &str) -> bool {
        matches!(
            tool_name,
            "shell" | "exec_command" | "apply_patch" | "edit_file_range"
        )
    }

    /// 获取沙箱描述
    pub fn sandbox_description(&self, sandbox_policy: SandboxPolicy) -> &'static str {
        sandbox_policy.as_str()
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}
