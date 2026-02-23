//! End-to-end tests for OpenJax CLI

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::Once;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        // Ensure cargo is built
        let _ = Command::new("cargo")
            .args(["build", "-p", "openjax-cli"])
            .output();
    });
}

#[test]
fn test_cli_help() {
    init();
    let output = Command::new("cargo")
        .args(["run", "-p", "openjax-cli", "--", "--help"])
        .output()
        .expect("failed to run openjax-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--model") || stdout.contains("MODEL"));
}

#[test]
fn test_cli_version() {
    init();
    let output = Command::new("cargo")
        .args(["run", "-p", "openjax-cli", "--", "--version"])
        .output()
        .expect("failed to run openjax-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.1.0") || stdout.contains("version"));
}

fn test_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("openjax-cli-{name}-{nanos}"));
    fs::create_dir_all(&dir).expect("failed to create temp test dir");
    dir
}

#[test]
fn test_cli_exit_command_smoke() {
    init();

    let temp_dir = test_temp_dir("exit");
    let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("debug")
        .join("openjax-cli");

    let mut child = Command::new(&bin_path)
        .current_dir(&temp_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn openjax-cli at {}: {e}", bin_path.display()));

    let mut stdin = child.stdin.take().expect("child stdin unavailable");
    stdin
        .write_all(b"/exit\n")
        .expect("failed to write /exit to stdin");
    drop(stdin);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    let output = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("openjax-cli did not exit within timeout")
        .expect("failed to collect process output");

    assert!(
        output.status.success(),
        "expected exit code 0, got: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        combined.contains("shutdown complete"),
        "expected shutdown message, got output:\n{combined}"
    );
    assert!(
        !combined.contains("panicked at"),
        "unexpected panic output:\n{combined}"
    );
    assert!(
        !combined.contains("thread 'main' panicked"),
        "unexpected panic output:\n{combined}"
    );
}
