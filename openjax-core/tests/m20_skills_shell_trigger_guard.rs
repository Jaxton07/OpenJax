use async_trait::async_trait;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::{ToolCall, ToolRouter, ToolRuntimeConfig};
use openjax_core::{ApprovalPolicy, SandboxMode};
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
        .execute(
            1,
            "test-call-1".to_string(),
            &call,
            &cwd,
            ToolRuntimeConfig {
                approval_policy: ApprovalPolicy::OnRequest,
                sandbox_mode: SandboxMode::WorkspaceWrite,
                prevent_shell_skill_trigger: true,
                ..ToolRuntimeConfig::default()
            },
            Arc::new(AllowAllApprovalHandler),
            None,
        )
        .await
        .expect("shell execution should return guard output");

    assert!(!outcome.success);
    assert!(
        outcome
            .output
            .contains("runtime_deny_reason=skill_trigger_not_shell_command")
    );
    assert!(
        outcome
            .output
            .contains("selected skill should be executed as workflow steps")
    );
}
