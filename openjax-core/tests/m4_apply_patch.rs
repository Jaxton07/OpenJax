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
    std::env::temp_dir().join(format!("openjax-m4-it-{pid}-{nanos}-{counter}"))
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

fn apply_patch_input(patch: &str) -> String {
    let escaped = patch.replace('\n', "\\n");
    format!("tool:apply_patch patch='{escaped}'")
}

#[tokio::test]
async fn apply_patch_add_update_delete_successfully() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\nline2\nline3").expect("seed file");
    fs::write(workspace.join("obsolete.txt"), "remove me").expect("seed delete file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let patch = "*** Begin Patch
*** Add File: notes.txt
+hello
*** Update File: todo.txt
@@
 line1
-line2
+line2-updated
 line3
*** Delete File: obsolete.txt
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(*ok);
            assert!(output.contains("patch applied successfully"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\nline2-updated\nline3");
    let notes = fs::read_to_string(workspace.join("notes.txt")).expect("notes should exist");
    assert_eq!(notes, "hello");
    assert!(!workspace.join("obsolete.txt").exists());

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn apply_patch_invalid_update_does_not_modify_file() {
    let workspace = create_workspace();
    fs::write(workspace.join("todo.txt"), "line1\nline2\nline3").expect("seed file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let patch = "*** Begin Patch
*** Update File: todo.txt
@@
 line1
-not-existing-line
+line2-updated
 line3
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(!ok);
            assert!(output.contains("failed to apply patch"));
        }
        _ => unreachable!(),
    }

    let todo = fs::read_to_string(workspace.join("todo.txt")).expect("todo should exist");
    assert_eq!(todo, "line1\nline2\nline3");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn apply_patch_rolls_back_when_later_action_fails() {
    let workspace = create_workspace();
    fs::write(workspace.join("blocker"), "I am a file").expect("seed blocker file");

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        workspace.clone(),
    );

    let patch = "*** Begin Patch
*** Add File: alpha.txt
+alpha
*** Add File: blocker/child.txt
+beta
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, .. } => {
            assert!(!ok);
        }
        _ => unreachable!(),
    }

    assert!(!workspace.join("alpha.txt").exists());
    let blocker = fs::read_to_string(workspace.join("blocker")).expect("blocker should remain");
    assert_eq!(blocker, "I am a file");

    let _ = fs::remove_dir_all(workspace);
}
