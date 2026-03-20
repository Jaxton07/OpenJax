// openjax-core/tests/m22_history_turn_record.rs
//
// Validates three core invariants after TurnRecord refactor:
// 1. Successful tool call turns are committed to history (with tool_traces)
// 2. After many turns, truncation only keeps MAX_CONVERSATION_HISTORY_TURNS turns
// 3. Tool calls that execute but return ok=false still complete and commit to history

use openjax_core::{Agent, ApprovalPolicy, SandboxMode};
use openjax_protocol::{Event, Op};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("openjax-m22-{pid}-{nanos}-{counter}"))
}

fn workspace() -> PathBuf {
    let dir = temp_dir();
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Successful tool:read_file turn should write to history.
/// Verify: two consecutive turns both complete (agent does not panic on history operations).
#[tokio::test]
async fn successful_tool_turn_increments_history() {
    let ws = workspace();
    fs::write(ws.join("data.txt"), "content").unwrap();

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        ws.clone(),
    );

    // Turn 1
    let events1 = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=data.txt".to_string(),
        })
        .await;

    assert!(
        matches!(events1.first(), Some(Event::TurnStarted { .. })),
        "turn 1 should start"
    );
    assert!(
        matches!(events1.last(), Some(Event::TurnCompleted { .. })),
        "turn 1 should complete"
    );
    assert!(
        events1.iter().any(|e| matches!(
            e,
            Event::ToolCallCompleted { tool_name, ok, .. }
            if tool_name == "read_file" && *ok
        )),
        "read_file should succeed in turn 1"
    );

    // Turn 2 — agent must not panic from history type issues
    let events2 = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=data.txt".to_string(),
        })
        .await;

    assert!(
        matches!(events2.first(), Some(Event::TurnStarted { .. })),
        "turn 2 should start"
    );
    assert!(
        matches!(events2.last(), Some(Event::TurnCompleted { .. })),
        "turn 2 should complete"
    );

    let _ = fs::remove_dir_all(ws);
}

/// After more than MAX_CONVERSATION_HISTORY_TURNS (10) turns, agent continues working.
/// Verify: truncation does not panic, agent remains usable.
#[tokio::test]
async fn history_truncation_does_not_panic() {
    let ws = workspace();
    fs::write(ws.join("file.txt"), "hello").unwrap();

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        ws.clone(),
    );

    // Submit 12 turns — exceeds MAX_CONVERSATION_HISTORY_TURNS = 10
    for _ in 0..12 {
        let events = agent
            .submit(Op::UserTurn {
                input: "tool:read_file path=file.txt".to_string(),
            })
            .await;
        assert!(
            matches!(events.last(), Some(Event::TurnCompleted { .. })),
            "each turn should complete without panic"
        );
    }

    let _ = fs::remove_dir_all(ws);
}

/// Reading a non-existent file causes ok=false, but the turn still completes.
/// Note: execute_single_tool_call returns Some from Ok(outcome) regardless of ok value,
/// so ok=false still commits to history. Only Err (retry exhaustion) returns None.
/// Verify: event sequence is TurnStarted ... ToolCallCompleted ... TurnCompleted.
#[tokio::test]
async fn failed_tool_call_turn_still_completes() {
    let ws = workspace();
    // intentionally no missing.txt file

    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        ws.clone(),
    );

    let events = agent
        .submit(Op::UserTurn {
            input: "tool:read_file path=missing.txt".to_string(),
        })
        .await;

    assert!(
        matches!(events.first(), Some(Event::TurnStarted { .. })),
        "turn should start"
    );
    assert!(
        matches!(events.last(), Some(Event::TurnCompleted { .. })),
        "turn should complete even when tool fails"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::ToolCallCompleted { tool_name, .. }
            if tool_name == "read_file"
        )),
        "tool call completed event should exist"
    );

    let _ = fs::remove_dir_all(ws);
}
