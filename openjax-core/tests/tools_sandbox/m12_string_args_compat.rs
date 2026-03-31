/// 验证 list_dir / read / grep_files / glob_files / shell 的数值参数同时接受 JSON 字符串和 JSON 数字。
use async_trait::async_trait;
use openjax_core::SandboxMode;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::{ToolCall, ToolExecutionRequest, ToolRouter, ToolRuntimeConfig};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct AlwaysApproveHandler;

#[async_trait]
impl ApprovalHandler for AlwaysApproveHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(true)
    }
}

fn call(name: &str, args: &[(&str, &str)]) -> ToolCall {
    let mut map = HashMap::new();
    for (k, v) in args {
        map.insert((*k).to_string(), (*v).to_string());
    }
    ToolCall {
        name: name.to_string(),
        args: map,
    }
}

fn temp_workspace() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("openjax-m12-{pid}-{nanos}-{counter}"));
    fs::create_dir_all(&path).expect("create workspace");
    path
}

fn exec_request<'a>(call: &'a ToolCall, cwd: &'a PathBuf) -> ToolExecutionRequest<'a> {
    ToolExecutionRequest {
        turn_id: 1,
        session_id: None,
        tool_call_id: "test".to_string(),
        call,
        cwd,
        config: ToolRuntimeConfig {
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..ToolRuntimeConfig::default()
        },
        approval_handler: Arc::new(AlwaysApproveHandler),
        event_sink: None,
        policy_runtime: None,
    }
}

// ── list_dir ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_dir_accepts_string_numeric_args() {
    let workspace = temp_workspace();
    fs::write(workspace.join("file.txt"), "hello").expect("seed");

    let router = ToolRouter::new();
    let tc = call(
        "list_dir",
        &[("dir_path", "."), ("offset", "1"), ("limit", "10"), ("depth", "2")],
    );
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("list_dir should execute");

    assert!(
        outcome.success,
        "list_dir with string args should succeed; output: {}",
        outcome.display_output
    );
    assert!(outcome.display_output.contains("file.txt"));

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn list_dir_accepts_default_args() {
    let workspace = temp_workspace();
    fs::write(workspace.join("file.txt"), "hello").expect("seed");

    let router = ToolRouter::new();
    let tc = call("list_dir", &[("dir_path", ".")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("list_dir default args should execute");

    assert!(outcome.success, "output: {}", outcome.display_output);

    let _ = fs::remove_dir_all(workspace);
}

// ── read ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_accepts_string_numeric_args() {
    let workspace = temp_workspace();
    fs::write(workspace.join("sample.txt"), "line1\nline2\nline3\n").expect("seed");

    let router = ToolRouter::new();
    let tc = call(
        "Read",
        &[("file_path", "sample.txt"), ("offset", "1"), ("limit", "2")],
    );
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("Read should execute");

    assert!(
        outcome.success,
        "Read with string args should succeed; output: {}",
        outcome.display_output
    );
    assert!(outcome.display_output.contains("line1"));

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn read_indentation_mode_accepts_string_numeric_args() {
    let workspace = temp_workspace();
    // 写入有缩进层级的内容，让 indentation 模式能正常工作
    fs::write(
        workspace.join("code.rs"),
        "fn main() {\n    let x = 1;\n    let y = 2;\n}\n",
    )
    .expect("seed");

    let router = ToolRouter::new();
    // 传入 mode=indentation 及字符串形式的 anchor_line / max_levels / max_lines
    let tc = call(
        "Read",
        &[
            ("file_path", "code.rs"),
            ("mode", "indentation"),
            ("anchor_line", "2"),
            ("max_levels", "3"),
            ("max_lines", "10"),
        ],
    );
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("Read indentation should execute");

    assert!(
        outcome.success,
        "Read indentation with string numeric args should succeed; output: {}",
        outcome.display_output
    );

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn read_default_args_succeed() {
    let workspace = temp_workspace();
    fs::write(workspace.join("sample.txt"), "hello\n").expect("seed");

    let router = ToolRouter::new();
    let tc = call("Read", &[("file_path", "sample.txt")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("Read should execute");

    assert!(outcome.success, "output: {}", outcome.display_output);

    let _ = fs::remove_dir_all(workspace);
}

// ── grep_files ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn grep_files_accepts_string_limit() {
    let workspace = temp_workspace();
    fs::write(workspace.join("notes.txt"), "hello world\nhello again\n").expect("seed");

    let router = ToolRouter::new();
    let tc = call("grep_files", &[("pattern", "hello"), ("limit", "5")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("grep_files should execute");

    assert!(
        outcome.success,
        "grep_files with string limit should succeed; output: {}",
        outcome.display_output
    );

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn grep_files_default_limit_succeeds() {
    let workspace = temp_workspace();
    fs::write(workspace.join("notes.txt"), "hello world\n").expect("seed");

    let router = ToolRouter::new();
    let tc = call("grep_files", &[("pattern", "hello")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("grep_files should execute");

    assert!(outcome.success, "output: {}", outcome.display_output);

    let _ = fs::remove_dir_all(workspace);
}

// ── shell ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn shell_accepts_string_timeout_ms() {
    let workspace = temp_workspace();
    let router = ToolRouter::new();
    let tc = call("shell", &[("cmd", "echo ok"), ("timeout_ms", "5000")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("shell should execute");

    assert!(
        outcome.success,
        "shell with string timeout_ms should succeed; output: {}",
        outcome.display_output
    );

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn shell_default_timeout_succeeds() {
    let workspace = temp_workspace();
    let router = ToolRouter::new();
    let tc = call("shell", &[("cmd", "echo ok")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("shell should execute");

    assert!(outcome.success, "output: {}", outcome.display_output);

    let _ = fs::remove_dir_all(workspace);
}

// ── glob_files ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn glob_files_accepts_string_limit() {
    let workspace = temp_workspace();
    fs::write(workspace.join("a.rs"), "fn main() {}").expect("seed");
    fs::write(workspace.join("b.rs"), "fn foo() {}").expect("seed");

    let router = ToolRouter::new();
    let tc = call("glob_files", &[("pattern", "*.rs"), ("limit", "5")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("glob_files should execute");

    assert!(
        outcome.success,
        "glob_files with string limit should succeed; output: {}",
        outcome.display_output
    );
    assert!(outcome.display_output.contains(".rs"));

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn glob_files_default_limit_succeeds() {
    let workspace = temp_workspace();
    fs::write(workspace.join("main.rs"), "fn main() {}").expect("seed");

    let router = ToolRouter::new();
    let tc = call("glob_files", &[("pattern", "*.rs")]);
    let outcome = router
        .execute(exec_request(&tc, &workspace))
        .await
        .expect("glob_files should execute");

    assert!(outcome.success, "output: {}", outcome.display_output);

    let _ = fs::remove_dir_all(workspace);
}
