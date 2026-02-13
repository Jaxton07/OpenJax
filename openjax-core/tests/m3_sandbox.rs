use openjax_core::{Agent, ApprovalPolicy, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
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
    std::env::temp_dir().join(format!("openjax-m3-it-{pid}-{nanos}-{counter}"))
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
                Event::ToolCallCompleted { tool_name: name, .. } if name == tool_name
            )
        })
        .expect("expected ToolCallCompleted event")
}

#[tokio::test]
async fn blocks_absolute_path_read_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=/etc/hosts".to_string(),
        })
        .await;

    match tool_completion(&events, "read_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("absolute paths are not allowed"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn blocks_parent_traversal_read_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=../outside.txt".to_string(),
        })
        .await;

    match tool_completion(&events, "read_file") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("parent traversal is not allowed"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn blocks_shell_redirect_write_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:exec_command cmd='echo hi >/tmp/openjax-e2e.txt'".to_string(),
        })
        .await;

    match tool_completion(&events, "exec_command") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("shell operators are not allowed"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn blocks_network_command_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:exec_command cmd='curl https://example.com'".to_string(),
        })
        .await;

    match tool_completion(&events, "exec_command") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("network/escalation command detected"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn allows_safe_readonly_command_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:exec_command cmd='ls -la'".to_string(),
        })
        .await;

    match tool_completion(&events, "exec_command") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("exit_code=0"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}
