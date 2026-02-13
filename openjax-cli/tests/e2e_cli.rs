//! End-to-end tests for OpenJax CLI

use std::process::Command;
use std::sync::Once;

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
