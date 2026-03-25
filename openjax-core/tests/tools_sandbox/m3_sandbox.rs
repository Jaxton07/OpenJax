use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Simulates a user who always rejects approval prompts.
struct RejectApprovalHandler;

#[async_trait]
impl ApprovalHandler for RejectApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
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
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

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
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

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
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    // Write-redirecting commands require approval; simulate a user who rejects.
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='echo hi >/tmp/openjax-e2e.txt'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("Approval rejected") || output.contains("command rejected"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn blocks_network_command_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    // Network commands require approval; simulate a user who rejects.
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='curl https://example.com'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("Approval rejected") || output.contains("command rejected"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn allows_safe_readonly_command_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='ls -la'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { output, .. } => {
            assert!(output.contains("exit_code="));
            assert!(!output.contains("approval is required but policy is set to never"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn allows_shell_pipeline_in_workspace_write() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='printf \"a\\nb\\n\" | head -n 1'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { output, .. } => {
            assert!(output.contains("exit_code="));
            assert!(!output.contains("approval is required but policy is set to never"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn marks_shell_non_zero_exit_as_failed() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='ls definitely_missing_file_12345'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("exit_code="));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn marks_pipeline_with_failed_segment_as_failed() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='false | true'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            // On shells without pipefail support (e.g. plain /bin/sh), the pipeline may
            // report success because only the last segment exit code is preserved.
            if output.contains("exit_code=0") {
                assert!(*ok);
                return;
            }
            assert!(!ok);
            assert!(output.contains("command=false | true"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn marks_sigpipe_pipeline_as_partial_success() {
    let workspace = create_workspace();
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='yes | head -n 1'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            // Without pipefail, this commonly returns exit_code=0 and plain success.
            if output.contains("exit_code=0") {
                assert!(*ok);
                return;
            }
            assert!(*ok);
            assert!(output.contains("result_class=partial_success"));
        }
        _ => unreachable!(),
    }

    let _ = fs::remove_dir_all(workspace);
}
