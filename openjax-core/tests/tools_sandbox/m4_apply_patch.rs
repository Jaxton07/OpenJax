use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

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

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

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

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

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

#[tokio::test]
async fn apply_patch_fuzzy_level1_trailing_whitespace() {
    let workspace = create_workspace();
    // "context_a" has trailing spaces in the file; the patch omits them.
    fs::write(
        workspace.join("src.txt"),
        "prefix\ncontext_a   \ncontext_b\nsuffix",
    )
    .expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    // Patch uses context line without trailing whitespace — should match via level-1 fuzzy.
    let patch = "*** Begin Patch
*** Update File: src.txt
@@
 prefix
 context_a
-context_b
+context_b-new
 suffix
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(
                *ok,
                "expected success with fuzzy level-1 match; output: {output}"
            );
            assert!(output.contains("patch applied successfully"));
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("src.txt")).expect("file should exist");
    // Original trailing whitespace on context_a is preserved; context_b is replaced.
    assert_eq!(content, "prefix\ncontext_a   \ncontext_b-new\nsuffix");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn apply_patch_fuzzy_level3_unicode_normalization() {
    let workspace = create_workspace();
    // File contains an em-dash; the patch uses a plain hyphen.
    fs::write(
        workspace.join("doc.txt"),
        "header\ntitle\u{2014}subtitle\nfooter",
    )
    .expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    // Patch uses plain "-" for the em-dash line — should match via level-3 unicode normalization.
    let patch = "*** Begin Patch
*** Update File: doc.txt
@@
 title-subtitle
-footer
+footer-new
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(
                *ok,
                "expected success with fuzzy level-3 match; output: {output}"
            );
            assert!(output.contains("patch applied successfully"));
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("doc.txt")).expect("file should exist");
    // The em-dash line is preserved as context; only footer is replaced.
    assert_eq!(content, "header\ntitle\u{2014}subtitle\nfooter-new");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn apply_patch_ambiguous_match_applies_to_first() {
    let workspace = create_workspace();
    // Duplicate context block appears twice; patch should apply to the first occurrence.
    fs::write(workspace.join("dup.txt"), "x = 1\ny = 2\n\nx = 1\ny = 2").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.set_approval_handler(Arc::new(AlwaysApproveHandler));

    let patch = "*** Begin Patch
*** Update File: dup.txt
@@
 x = 1
-y = 2
+y = 99
*** End Patch";

    let events = agent
        .submit(Op::UserTurn {
            input: apply_patch_input(patch),
        })
        .await;

    match tool_completion(&events, "apply_patch") {
        Event::ToolCallCompleted { ok, output, .. } => {
            assert!(
                *ok,
                "expected success despite ambiguous match; output: {output}"
            );
            assert!(output.contains("patch applied successfully"));
            // Warning about multiple matches should be present in the response.
            assert!(
                output.contains("matched multiple locations"),
                "expected ambiguous-match warning in output; got: {output}"
            );
        }
        _ => unreachable!(),
    }

    let content = fs::read_to_string(workspace.join("dup.txt")).expect("file should exist");
    // First occurrence replaced, second unchanged.
    assert_eq!(content, "x = 1\ny = 99\n\nx = 1\ny = 2");

    let _ = fs::remove_dir_all(workspace);
}
