use openjax_core::{Agent, SandboxMode};
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
    std::env::temp_dir().join(format!("openjax-m7-compat-it-{pid}-{nanos}-{counter}"))
}

fn create_workspace() -> PathBuf {
    let workspace = temp_workspace_path();
    fs::create_dir_all(&workspace).expect("failed to create temp workspace");
    workspace
}

#[tokio::test]
async fn submit_still_returns_full_turn_event_sequence() {
    let workspace = create_workspace();
    fs::write(workspace.join("note.txt"), "hello\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Read path=note.txt".to_string(),
        })
        .await;

    assert!(matches!(events.first(), Some(Event::TurnStarted { .. })));
    assert!(matches!(events.last(), Some(Event::TurnCompleted { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, Event::ToolCallCompleted { tool_name, ok, .. } if tool_name == "Read" && *ok)));
    let started_id = events.iter().find_map(|event| match event {
        Event::ToolCallStarted {
            tool_name,
            tool_call_id,
            ..
        } if tool_name == "Read" => Some(tool_call_id.as_str()),
        _ => None,
    });
    let completed_id = events.iter().find_map(|event| match event {
        Event::ToolCallCompleted {
            tool_name,
            tool_call_id,
            ..
        } if tool_name == "Read" => Some(tool_call_id.as_str()),
        _ => None,
    });
    assert_eq!(started_id, completed_id);

    let _ = fs::remove_dir_all(workspace);
}
