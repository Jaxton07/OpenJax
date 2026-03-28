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
    std::env::temp_dir().join(format!("openjax-m10-it-{pid}-{nanos}-{counter}"))
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
async fn write_file_creates_new_file_inside_workspace() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:write_file file_path=notes/todo.txt content='hello-from-m10'".to_string(),
        })
        .await;

    match tool_completion(&events, "write_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("written"));
            assert!(output.contains("notes/todo.txt"));
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("notes/todo.txt")).expect("new file should exist");
    assert_eq!(content, "hello-from-m10");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn write_file_overwrites_existing_file() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "old-content").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:write_file file_path=todo.txt content='new-content'".to_string(),
        })
        .await;

    match tool_completion(&events, "write_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("written"));
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("todo.txt")).expect("updated file should exist");
    assert_eq!(content, "new-content");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn write_file_rejects_workspace_escape() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let parent = workspace
        .parent()
        .expect("workspace temp dir should have a parent");
    let escaped_name = format!("openjax-m10-escape-{}.txt", std::process::id());
    let escaped_path = parent.join(&escaped_name);
    let _ = fs::remove_file(&escaped_path);

    let input = format!(
        "tool:write_file file_path=../{} content='nope'",
        escaped_name
    );
    let events = agent.submit(Op::UserTurn { input }).await;

    match tool_completion(&events, "write_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(
                output.contains("parent traversal is not allowed")
                    || output.contains("outside workspace")
            );
        }
        _ => unreachable!(),
    }

    assert!(!escaped_path.exists());
    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn write_file_creates_missing_parent_directories() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:write_file file_path=nested/a/b/out.txt content='auto-parent-dir'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "write_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("nested/a/b/out.txt"));
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("nested/a/b/out.txt"))
        .expect("file should exist in newly created parent dirs");
    assert_eq!(content, "auto-parent-dir");

    let _ = fs::remove_dir_all(workspace);
}
