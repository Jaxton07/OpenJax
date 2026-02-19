use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalPolicy, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_workspace_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!(
        "openjax-m8-approval-events-it-{pid}-{nanos}-{counter}"
    ))
}

fn create_workspace() -> PathBuf {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("failed to create temp workspace");
    workspace
}

#[derive(Debug)]
struct AlwaysApprove;

#[async_trait]
impl ApprovalHandler for AlwaysApprove {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(true)
    }
}

#[tokio::test]
async fn emits_approval_requested_and_resolved_events() {
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::AlwaysAsk,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    agent.set_approval_handler(Arc::new(AlwaysApprove));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=note.txt".to_string(),
        })
        .await;

    let requested = events
        .iter()
        .find(|event| matches!(event, Event::ApprovalRequested { .. }));
    let resolved = events
        .iter()
        .find(|event| matches!(event, Event::ApprovalResolved { .. }));
    assert!(requested.is_some());
    assert!(resolved.is_some());

    let _ = fs::remove_dir_all(workspace);
}
