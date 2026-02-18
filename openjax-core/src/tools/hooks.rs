use crate::tools::events::HookEvent;
use tracing::{debug, info};

/// 钩子执行器
pub struct HookExecutor;

impl HookExecutor {
    pub fn new() -> Self {
        Self
    }

    /// 执行钩子事件
    pub fn execute(&self, event: &HookEvent) {
        match event {
            HookEvent::BeforeToolUse(data) => {
                debug!(
                    tool_name = %data.tool_name,
                    call_id = %data.call_id,
                    "BeforeToolUse: {}", data.tool_input
                );
            }
            HookEvent::AfterToolUse(data) => {
                info!(
                    tool_name = %data.tool_name,
                    call_id = %data.call_id,
                    success = data.success,
                    duration_ms = data.duration_ms,
                    "AfterToolUse: executed={}, mutating={}",
                    data.executed, data.mutating
                );
            }
        }
    }
}

impl Default for HookExecutor {
    fn default() -> Self {
        Self::new()
    }
}
