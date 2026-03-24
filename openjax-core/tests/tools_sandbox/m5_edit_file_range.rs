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
async fn edit_file_range_replaces_lines_successfully() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\nline2\nline3\nline4\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let input = "tool:edit_file_range file_path=todo.txt start_line=2 end_line=3 new_text='line2-updated\nline3-updated'";
    let events = agent
        .submit(Op::UserTurn {
            input: input.to_string(),
        })
        .await;

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("edit applied successfully"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\nline2-updated\nline3-updated\nline4\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn edit_file_range_deletes_lines_with_empty_text() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\nb\nc\nd\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let input = "tool:edit_file_range file_path=todo.txt start_line=2 end_line=3 new_text=''";
    let events = agent
        .submit(Op::UserTurn {
            input: input.to_string(),
        })
        .await;

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, .. } => {
            assert!(*ok);
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "a\nd\n");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn edit_file_range_rejects_invalid_range() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());

    let input = "tool:edit_file_range file_path=todo.txt start_line=3 end_line=4 new_text='x'";
    let events = agent
        .submit(Op::UserTurn {
            input: input.to_string(),
        })
        .await;

    match tool_completion(&events, "edit_file_range") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("exceeds file length"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "a\nb\n");

    let _ = fs::remove_dir_all(workspace);
}
