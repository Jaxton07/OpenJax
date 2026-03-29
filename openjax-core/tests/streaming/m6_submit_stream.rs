use openjax_core::{Agent, SandboxMode};
use openjax_protocol::Op;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::unbounded_channel;

fn temp_workspace_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("openjax-m6-stream-it-{pid}-{nanos}-{counter}"))
}

fn create_workspace() -> PathBuf {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("failed to create temp workspace");
    workspace
}

#[tokio::test]
async fn submit_with_sink_emits_events_in_same_order_as_submit_result() {
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    let (tx, mut rx) = unbounded_channel();

    let returned_events = agent
        .submit_with_sink(
            Op::UserTurn {
                input: "tool:Read path=note.txt".to_string(),
            },
            tx,
        )
        .await;

    let mut streamed_events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        streamed_events.push(event);
    }

    assert_eq!(streamed_events, returned_events);
    assert!(!streamed_events.is_empty());

    let _ = fs::remove_dir_all(workspace);
}
