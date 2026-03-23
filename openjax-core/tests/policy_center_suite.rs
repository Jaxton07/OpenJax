use std::sync::Arc;

use async_trait::async_trait;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::context::{
    ApprovalPolicy, SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext,
};
use openjax_core::tools::error::FunctionCallError;
use openjax_core::tools::orchestrator::ToolOrchestrator;
use openjax_core::tools::registry::ToolRegistry;
use openjax_core::tools::shell::ShellType;

#[derive(Debug)]
struct RejectApproval;

#[async_trait]
impl ApprovalHandler for RejectApproval {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
    }
}

#[tokio::test]
async fn unknown_tool_without_descriptor_defaults_to_ask() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let invocation = ToolInvocation {
        tool_name: "unknown_tool".to_string(),
        call_id: "call-unknown-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 42,
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            approval_policy: ApprovalPolicy::OnRequest,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: None,
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = orchestrator.run(invocation).await;
    match result {
        Err(FunctionCallError::ApprovalRejected(_)) => {}
        other => panic!("expected ApprovalRejected for unknown descriptor, got: {other:?}"),
    }
}
