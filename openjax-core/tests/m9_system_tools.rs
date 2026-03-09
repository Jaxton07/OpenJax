use async_trait::async_trait;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::{self, ToolCall, ToolRouter, ToolRuntimeConfig};
use openjax_core::{ApprovalPolicy, SandboxMode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct CountingApprovalHandler {
    calls: AtomicUsize,
}

#[async_trait]
impl ApprovalHandler for CountingApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }
}

fn call(name: &str, args: &[(&str, &str)]) -> ToolCall {
    let mut map = HashMap::new();
    for (k, v) in args {
        map.insert((*k).to_string(), (*v).to_string());
    }
    ToolCall {
        name: name.to_string(),
        args: map,
    }
}

#[tokio::test]
async fn system_tools_are_registered_in_specs() {
    let specs = tools::build_all_specs(&tools::ToolsConfig::default());
    let names: Vec<String> = specs.into_iter().map(|s| s.name).collect();
    assert!(names.contains(&"process_snapshot".to_string()));
    assert!(names.contains(&"system_load".to_string()));
    assert!(names.contains(&"disk_usage".to_string()));
}

#[tokio::test]
async fn process_snapshot_dispatch_returns_json() {
    let router = ToolRouter::new();
    let cwd = PathBuf::from(".");
    let approval = Arc::new(CountingApprovalHandler::default());

    let outcome = router
        .execute(
            1,
            "test-call-1".to_string(),
            &call("process_snapshot", &[("limit", "5"), ("sort_by", "cpu")]),
            &cwd,
            ToolRuntimeConfig {
                approval_policy: ApprovalPolicy::OnRequest,
                sandbox_mode: SandboxMode::WorkspaceWrite,
                ..ToolRuntimeConfig::default()
            },
            approval.clone(),
            None,
        )
        .await
        .expect("process_snapshot should execute");

    assert!(outcome.success);
    let json: serde_json::Value = serde_json::from_str(&outcome.output).expect("json output");
    assert!(json.get("items").is_some());
    assert_eq!(approval.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn system_load_does_not_trigger_approval_under_on_request() {
    let router = ToolRouter::new();
    let cwd = PathBuf::from(".");
    let approval = Arc::new(CountingApprovalHandler::default());

    let outcome = router
        .execute(
            1,
            "test-call-2".to_string(),
            &call(
                "system_load",
                &[("include_cpu", "true"), ("include_memory", "false")],
            ),
            &cwd,
            ToolRuntimeConfig {
                approval_policy: ApprovalPolicy::OnRequest,
                sandbox_mode: SandboxMode::WorkspaceWrite,
                ..ToolRuntimeConfig::default()
            },
            approval.clone(),
            None,
        )
        .await
        .expect("system_load should execute");

    assert!(outcome.success);
    assert_eq!(approval.calls.load(Ordering::Relaxed), 0);
}
