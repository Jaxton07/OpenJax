use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio::time::{Duration, timeout};

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

#[derive(Debug)]
struct BlockingApprove {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl ApprovalHandler for BlockingApprove {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(true)
    }
}

#[tokio::test]
async fn emits_approval_requested_and_resolved_events() {
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
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

    let Event::ApprovalRequested { approval_kind, .. } =
        requested.expect("ApprovalRequested event should exist")
    else {
        panic!("event must be ApprovalRequested");
    };
    assert_eq!(approval_kind, &Some(openjax_protocol::ApprovalKind::Normal));

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn submit_with_sink_emits_approval_requested_before_resolution() {
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(BlockingApprove {
        entered: entered.clone(),
        release: release.clone(),
    }));

    let (sink_tx, mut sink_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let submit_task = tokio::spawn(async move {
        agent
            .submit_with_sink(
                Op::UserTurn {
                    input: "tool:read_file path=note.txt".to_string(),
                },
                sink_tx,
            )
            .await
    });

    timeout(Duration::from_secs(2), entered.notified())
        .await
        .expect("approval handler should be entered");

    let approval_evt = timeout(Duration::from_secs(1), async {
        loop {
            let evt = sink_rx
                .recv()
                .await
                .expect("sink should receive events while submit is blocked");
            if matches!(evt, Event::ApprovalRequested { .. }) {
                break evt;
            }
        }
    })
    .await
    .expect("approval_requested should stream before approval resolution");

    assert!(matches!(approval_evt, Event::ApprovalRequested { .. }));

    let Event::ApprovalRequested { approval_kind, .. } = &approval_evt else {
        panic!("event must be ApprovalRequested");
    };
    assert_eq!(approval_kind, &Some(openjax_protocol::ApprovalKind::Normal));

    release.notify_one();
    let events = submit_task.await.expect("submit task join");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::ApprovalResolved { .. })),
        "submit should complete after approval is released"
    );

    let _ = fs::remove_dir_all(workspace);
}
