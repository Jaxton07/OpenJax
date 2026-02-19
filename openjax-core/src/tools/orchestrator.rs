use crate::approval::ApprovalRequest;
use crate::tools::ToolsConfig;
use crate::tools::context::{ApprovalPolicy, ToolInvocation, ToolOutput};
use crate::tools::events::{AfterToolUse, BeforeToolUse, HookEvent};
use crate::tools::hooks::HookExecutor;
use crate::tools::registry::ToolRegistry;
use crate::tools::sandboxing::SandboxManager;
use std::sync::Arc;
use std::time::Instant;

/// 工具编排器
pub struct ToolOrchestrator {
    registry: Arc<ToolRegistry>,
    hook_executor: HookExecutor,
    sandbox_manager: SandboxManager,
    _config: ToolsConfig,
}

impl ToolOrchestrator {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            hook_executor: HookExecutor::new(),
            sandbox_manager: SandboxManager::new(),
            _config: ToolsConfig::default(),
        }
    }

    pub fn with_config(registry: Arc<ToolRegistry>, config: ToolsConfig) -> Self {
        Self {
            registry,
            hook_executor: HookExecutor::new(),
            sandbox_manager: SandboxManager::new(),
            _config: config,
        }
    }

    /// 注册动态工具
    pub fn register_tool(
        &self,
        name: String,
        handler: Arc<dyn crate::tools::registry::ToolHandler>,
    ) {
        let registry = Arc::as_ptr(&self.registry) as *const ToolRegistry as *mut ToolRegistry;
        unsafe {
            (*registry).register(name, handler);
        }
    }

    /// 执行工具调用
    pub async fn run(
        &self,
        invocation: ToolInvocation,
    ) -> Result<ToolOutput, crate::tools::error::FunctionCallError> {
        // 1. 执行前钩子
        self.hook_executor
            .execute(&HookEvent::BeforeToolUse(BeforeToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
            }));

        // 2. 检查是否需要批准
        let is_mutating = self
            .sandbox_manager
            .is_mutating_operation(&invocation.tool_name);
        let requires_approval =
            self.should_prompt_approval(invocation.turn.approval_policy, is_mutating);

        if requires_approval {
            let approved = invocation
                .turn
                .approval_handler
                .request_approval(ApprovalRequest {
                    target: invocation.tool_name.clone(),
                    reason: "tool call requires approval by policy".to_string(),
                })
                .await
                .map_err(crate::tools::error::FunctionCallError::Internal)?;

            if !approved {
                return Err(crate::tools::error::FunctionCallError::ApprovalRejected(
                    "command rejected by user".to_string(),
                ));
            }
        }

        // 3. 选择合适的沙箱
        let sandbox = self
            .sandbox_manager
            .select_sandbox(invocation.turn.sandbox_policy);

        // 4. 执行工具
        let start = Instant::now();
        let result = self.registry.dispatch(invocation.clone()).await;
        let duration = start.elapsed();

        // 5. 执行后钩子
        let is_success = result.is_ok();
        let output_preview = result.as_ref().ok().map(|o| format!("{:?}", o));

        self.hook_executor
            .execute(&HookEvent::AfterToolUse(AfterToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
                executed: is_success,
                success: is_success,
                duration_ms: duration.as_millis() as u64,
                mutating: is_mutating,
                sandbox: sandbox.as_str().to_string(),
                sandbox_policy: invocation.turn.sandbox_policy.as_str().to_string(),
                output_preview,
            }));

        result
    }

    fn should_prompt_approval(&self, policy: ApprovalPolicy, is_mutating: bool) -> bool {
        match policy {
            ApprovalPolicy::AlwaysAsk => true,
            ApprovalPolicy::OnRequest => is_mutating,
            ApprovalPolicy::Never => false,
        }
    }
}
