use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
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
    std::env::temp_dir().join(format!("openjax-m21-tool-stream-{pid}-{nanos}-{counter}"))
}

#[derive(Debug)]
struct RejectApproval;

#[async_trait]
impl ApprovalHandler for RejectApproval {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
    }
}

#[tokio::test]
async fn emits_args_delta_and_progress_before_completion() {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=note.txt".to_string(),
        })
        .await;

    let started = events
        .iter()
        .position(|evt| matches!(evt, Event::ToolCallStarted { .. }))
        .expect("tool_call_started");
    let args_delta = events
        .iter()
        .position(|evt| matches!(evt, Event::ToolCallArgsDelta { .. }))
        .expect("tool_args_delta");
    let progress = events
        .iter()
        .position(|evt| matches!(evt, Event::ToolCallProgress { .. }))
        .expect("tool_call_progress");
    let ready = events
        .iter()
        .position(|evt| matches!(evt, Event::ToolCallReady { .. }))
        .expect("tool_call_ready");
    let completed = events
        .iter()
        .position(|evt| matches!(evt, Event::ToolCallCompleted { .. }))
        .expect("tool_call_completed");

    assert!(started < args_delta);
    assert!(args_delta < ready);
    assert!(ready < progress);
    assert!(progress < completed);

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn approval_rejection_emits_tool_call_failed() {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(RejectApproval));

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:edit_file_range file_path=todo.txt start_line=1 end_line=1 new_text='x'"
                .to_string(),
        })
        .await;

    assert!(events.iter().any(|evt| matches!(
        evt,
        Event::ApprovalResolved {
            approved: false,
            ..
        }
    )));

    let failed_event = events
        .iter()
        .find(|evt| matches!(evt, Event::ToolCallFailed { .. }))
        .expect("tool_call_failed");
    assert!(matches!(
        failed_event,
        Event::ToolCallFailed {
            code,
            retryable,
            ..
        } if code == "approval_rejected" && !retryable
    ));

    let _ = fs::remove_dir_all(workspace);
}
