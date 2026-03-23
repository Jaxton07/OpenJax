#![allow(clippy::await_holding_lock)]

use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalPolicy, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, sleep};

static APPROVAL_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn serial_guard() -> MutexGuard<'static, ()> {
    APPROVAL_TEST_LOCK
        .lock()
        .expect("approval test lock poisoned")
}

fn temp_workspace_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("openjax-m5-approval-it-{pid}-{nanos}-{counter}"))
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

#[derive(Debug)]
struct MockApprovalHandler {
    calls: AtomicUsize,
    decisions: Mutex<Vec<bool>>,
}

#[derive(Debug)]
struct BlockingApprovalHandler;

#[async_trait]
impl ApprovalHandler for BlockingApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        sleep(Duration::from_secs(60)).await;
        Ok(true)
    }
}

impl MockApprovalHandler {
    fn new(decisions: Vec<bool>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            decisions: Mutex::new(decisions),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ApprovalHandler for MockApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let mut guard = self
            .decisions
            .lock()
            .map_err(|_| "approval mutex poisoned".to_string())?;
        Ok(guard.pop().unwrap_or(false))
    }
}

#[tokio::test]
async fn always_ask_prompts_and_approved_call_succeeds() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::AlwaysAsk,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    let handler = Arc::new(MockApprovalHandler::new(vec![true]));
    agent.set_approval_handler(handler.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=note.txt".to_string(),
        })
        .await;

    match tool_completion(&events, "read_file") {
        Event::ToolCallCompleted { ok, .. } => assert!(*ok),
        _ => unreachable!(),
    }
    assert_eq!(handler.call_count(), 1);

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn on_request_skips_prompt_for_non_mutating_tool() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::OnRequest,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    let handler = Arc::new(MockApprovalHandler::new(vec![]));
    agent.set_approval_handler(handler.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=note.txt".to_string(),
        })
        .await;

    match tool_completion(&events, "read_file") {
        Event::ToolCallCompleted { ok, .. } => assert!(*ok),
        _ => unreachable!(),
    }
    assert_eq!(handler.call_count(), 0);

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn on_request_prompts_for_mutating_tool_and_rejects() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::OnRequest,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    let handler = Arc::new(MockApprovalHandler::new(vec![false]));
    agent.set_approval_handler(handler.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:edit_file_range file_path=todo.txt start_line=1 end_line=1 new_text='x'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("Approval rejected"));
        }
        _ => unreachable!(),
    }
    assert_eq!(handler.call_count(), 1);

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "a\nb\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn never_does_not_prompt_even_for_mutating_tool() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    let handler = Arc::new(MockApprovalHandler::new(vec![false]));
    agent.set_approval_handler(handler.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:edit_file_range file_path=todo.txt start_line=1 end_line=1 new_text='x'"
                .to_string(),
        })
        .await;

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, .. } => assert!(*ok),
        _ => unreachable!(),
    }
    assert_eq!(handler.call_count(), 0);

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "x\nb\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn approval_timeout_is_reported_as_timeout() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::OnRequest,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    agent.set_approval_handler(Arc::new(BlockingApprovalHandler));

    unsafe {
        std::env::set_var("OPENJAX_APPROVAL_TIMEOUT_MS", "20");
    }

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:edit_file_range file_path=todo.txt start_line=1 end_line=1 new_text='x'"
                .to_string(),
        })
        .await;

    unsafe {
        std::env::remove_var("OPENJAX_APPROVAL_TIMEOUT_MS");
    }

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("Approval timed out"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "a\nb\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn on_request_prompts_for_git_commit_and_rejects() {
    let _guard = serial_guard();
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\n").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::OnRequest,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );
    let handler = Arc::new(MockApprovalHandler::new(vec![false]));
    agent.set_approval_handler(handler.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:shell cmd='git commit -m \"test\"'".to_string(),
        })
        .await;

    match tool_completion(&events, "shell") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("Approval rejected"));
        }
        _ => unreachable!(),
    }
    assert_eq!(handler.call_count(), 1);

    let _ = fs::remove_dir_all(workspace);
}
