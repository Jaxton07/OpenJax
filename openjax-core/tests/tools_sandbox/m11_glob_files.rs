use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    std::env::temp_dir().join(format!("openjax-m11-it-{pid}-{nanos}-{counter}"))
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
async fn glob_files_returns_matches_sorted_newest_first() {
    let workspace = create_workspace();
    fs::create_dir_all(workspace.join("logs")).expect("create logs dir");
    fs::write(workspace.join("logs/older.log"), "older").expect("seed older file");
    sleep(Duration::from_millis(1200));
    fs::write(workspace.join("logs/newer.log"), "newer").expect("seed newer file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:glob_files pattern='logs/*.log'".to_string(),
        })
        .await;

    match tool_completion(&events, "glob_files") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            let newer_index = output
                .find("logs/newer.log")
                .expect("newer match should be present");
            let older_index = output
                .find("logs/older.log")
                .expect("older match should be present");
            assert!(
                newer_index < older_index,
                "expected newer.log before older.log, output: {output}"
            );
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn glob_files_rejects_workspace_escape() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:glob_files pattern='*.rs' base_path=../".to_string(),
        })
        .await;

    match tool_completion(&events, "glob_files") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(
                output.contains("parent traversal is not allowed")
                    || output.contains("outside workspace")
            );
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn glob_files_respects_limit() {
    let workspace = create_workspace();
    fs::create_dir_all(workspace.join("src")).expect("create src dir");
    fs::write(workspace.join("src/limit_a.rs"), "fn a() {}").expect("seed a");
    sleep(Duration::from_millis(1200));
    fs::write(workspace.join("src/limit_b.rs"), "fn b() {}").expect("seed b");
    sleep(Duration::from_millis(1200));
    fs::write(workspace.join("src/limit_c.rs"), "fn c() {}").expect("seed c");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:glob_files pattern='src/*.rs' limit=2".to_string(),
        })
        .await;

    match tool_completion(&events, "glob_files") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            let files = ["src/limit_a.rs", "src/limit_b.rs", "src/limit_c.rs"];
            let matched = files.iter().filter(|name| output.contains(**name)).count();
            assert_eq!(matched, 2, "expected exactly 2 matched files, output: {output}");
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn glob_files_returns_empty_when_nothing_matches() {
    let workspace = create_workspace();
    fs::create_dir_all(workspace.join("notes")).expect("create notes dir");
    fs::write(workspace.join("notes/todo.txt"), "todo").expect("seed txt file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:glob_files pattern='**/*.definitely_not_real'".to_string(),
        })
        .await;

    match tool_completion(&events, "glob_files") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(
                output.trim().is_empty(),
                "expected empty output when no files match, got: {output}"
            );
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}
