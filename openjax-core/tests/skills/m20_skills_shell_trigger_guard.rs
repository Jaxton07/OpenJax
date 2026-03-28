use async_trait::async_trait;
use openjax_core::SandboxMode;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::{ToolCall, ToolExecutionRequest, ToolRouter, ToolRuntimeConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
struct AllowAllApprovalHandler;

#[async_trait]
impl ApprovalHandler for AllowAllApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(true)
    }
}

#[tokio::test]
async fn shell_guard_blocks_skill_trigger_like_command() {
    let router = ToolRouter::new();
    let cwd = PathBuf::from(".");
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "/abc-skill".to_string());
    let call = ToolCall {
        name: "shell".to_string(),
        args,
    };

    let outcome = router
        .execute(ToolExecutionRequest {
            turn_id: 1,
            session_id: None,
            tool_call_id: "test-call-1".to_string(),
            call: &call,
            cwd: &cwd,
            config: ToolRuntimeConfig {
                sandbox_mode: SandboxMode::WorkspaceWrite,
                prevent_shell_skill_trigger: true,
                ..ToolRuntimeConfig::default()
            },
            approval_handler: Arc::new(AllowAllApprovalHandler),
            event_sink: None,
            policy_runtime: None,
        })
        .await
        .expect("shell execution should return guard output");

    assert!(!outcome.success);
    assert!(
        outcome
            .display_output
            .contains("runtime_deny_reason=skill_trigger_not_shell_command")
    );
    assert!(
        outcome
            .display_output
            .contains("selected skill should be executed as workflow steps")
    );
}
