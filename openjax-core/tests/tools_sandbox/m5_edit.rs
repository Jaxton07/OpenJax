use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct AlwaysApproveHandler;

#[async_trait]
impl ApprovalHandler for AlwaysApproveHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(true)
    }
}

fn temp_workspace_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("openjax-m5-it-{pid}-{nanos}-{counter}"))
}

fn create_workspace() -> PathBuf {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("failed to create temp workspace");
    workspace
}

fn tool_completion<'a>(events: &'a [Event], tool_name: &str) -> &'a Event {
    events
        .iter()
        .find(|event| {
            matches!(
                event,
                Event::ToolCallCompleted {
                    tool_name: name,
                    ..
                } if name == tool_name
            )
        })
        .expect("expected ToolCallCompleted event")
}

#[tokio::test]
async fn edit_replaces_unique_match_successfully() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\nline2\nline3\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Edit file_path=todo.txt old_string='line2' new_string='line2-updated'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "Edit") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(
                *ok,
                "expected Edit unique replacement to succeed; output: {output}"
            );
            assert!(output.contains("updated successfully"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\nline2-updated\nline3\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn edit_returns_not_found_when_old_string_is_missing() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\nline2\nline3\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Edit file_path=todo.txt old_string='line4' new_string='line4-updated'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "Edit") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(
                output.contains("Edit failed [not_found]"),
                "expected not_found failure class; output: {output}"
            );
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\nline2\nline3\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn edit_returns_not_unique_for_multiple_matches() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "repeat\nline2\nrepeat\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Edit file_path=todo.txt old_string='repeat' new_string='updated-repeat'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "Edit") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(
                output.contains("Edit failed [not_unique]"),
                "expected not_unique failure class; output: {output}"
            );
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "repeat\nline2\nrepeat\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn edit_normalizes_newlines_before_matching() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\r\nline2\r\nline3\r\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input:
                "tool:Edit file_path=todo.txt old_string='line2\nline3' new_string='line2-updated\nline3-updated'"
                    .to_string(),
        })
        .await;

    match tool_completion(&events, "Edit") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(
                *ok,
                "expected newline-normalized matching to succeed; output: {output}"
            );
            assert!(output.contains("updated successfully"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\r\nline2-updated\r\nline3-updated\r\n");

    let _ = fs::remove_dir_all(workspace);
}
