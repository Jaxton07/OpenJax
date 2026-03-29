//! Aggregated integration suite for approval event emission flows.

#[path = "approval/m8_approval_event_emission.rs"]
mod approval_event_emission_m8;

use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_policy::{runtime::PolicyRuntime, schema::DecisionKind, store::PolicyStore};
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
        "openjax-approval-events-suite-it-{pid}-{nanos}-{counter}"
    ))
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
async fn approval_event_contains_policy_metadata() {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("failed to create temp workspace");
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let policy_runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_policy_runtime(Some(policy_runtime));
    agent.set_approval_handler(Arc::new(AlwaysApprove));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Read path=note.txt".to_string(),
        })
        .await;

    let (policy_version, matched_rule_id, approval_kind) = events
        .iter()
        .find_map(|event| match event {
            Event::ApprovalRequested {
                policy_version,
                matched_rule_id,
                approval_kind,
                ..
            } => Some((
                *policy_version,
                matched_rule_id.clone(),
                approval_kind.clone(),
            )),
            _ => None,
        })
        .expect("approval event should be emitted");

    assert_eq!(policy_version, Some(1));
    // No custom rules in store → default Ask applies, no matched_rule_id
    assert_eq!(matched_rule_id, None);
    assert_eq!(approval_kind, Some(openjax_protocol::ApprovalKind::Normal));

    let _ = fs::remove_dir_all(workspace);
}
