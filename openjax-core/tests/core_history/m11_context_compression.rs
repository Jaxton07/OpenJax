// openjax-core/tests/core_history/m11_context_compression.rs
//
// Integration tests for context compression functionality.
// Tests verify:
// 1. Short history (4 or fewer turns) skips compression
// 2. Manual compact produces Summary + recent turns
// 3. Zero context_window_size skips auto-compaction

use openjax_core::{
    Agent, Config, ModelConfig, ModelRoutingConfig, ProviderModelConfig, SandboxMode,
};
use openjax_protocol::{Event, Op};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static CWD_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct CwdScope {
    _lock: MutexGuard<'static, ()>,
    original: PathBuf,
}

impl CwdScope {
    fn enter(target: &Path) -> Self {
        let lock = CWD_LOCK.lock().expect("cwd lock poisoned");
        let original = std::env::current_dir().expect("current dir should be available");
        std::env::set_current_dir(target).expect("set current dir");
        Self {
            _lock: lock,
            original,
        }
    }
}

impl Drop for CwdScope {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("openjax-m11-{pid}-{nanos}-{counter}"))
}

fn workspace() -> PathBuf {
    let dir = temp_dir();
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Verify split_for_compression returns None when Turn count <= 4.
/// With 4 or fewer turns, no ContextCompacted event should be emitted.
#[tokio::test]
async fn test_split_for_compression_skips_short_history() {
    let ws = workspace();
    fs::write(ws.join("file.txt"), "content").unwrap();

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, ws.clone());

    // Submit exactly 4 turns (the boundary case)
    for i in 0..4 {
            let events = agent
                .submit(Op::UserTurn {
                    input: "tool:Read path=file.txt".to_string(),
                })
                .await;

        assert!(
            matches!(events.last(), Some(Event::TurnCompleted { .. })),
            "turn {} should complete",
            i
        );

        // No ContextCompacted event should be emitted for <= 4 turns
        let has_compacted = events
            .iter()
            .any(|e| matches!(e, Event::ContextCompacted { .. }));
        assert!(
            !has_compacted,
            "turn {} should NOT trigger compression (4 or fewer turns)",
            i
        );
    }

    // Now add one more turn (5 total)
    let events = agent
        .submit(Op::UserTurn {
            input: "tool:Read path=file.txt".to_string(),
        })
        .await;

    // Still no auto-compaction should happen (context_window_size defaults to 0)
    let has_compacted = events
        .iter()
        .any(|e| matches!(e, Event::ContextCompacted { .. }));
    assert!(
        !has_compacted,
        "with context_window_size=0 (default), auto-compact should not trigger"
    );

    let _ = fs::remove_dir_all(ws);
}

/// Verify that manual compact produces Summary + recent turns.
/// After compression, the first history item should be Summary,
/// followed by 3 recent Turns.
#[tokio::test]
async fn test_compact_produces_summary_plus_recent() {
    let ws = workspace();
    fs::write(ws.join("file.txt"), "content").unwrap();

    // Serialize cwd mutations in this integration suite.
    let _cwd_scope = CwdScope::enter(&ws);

    // Create config with echo backend to allow compression without real API key
    let model_config = ModelConfig {
        backend: Some("echo".to_string()),
        api_key: None,
        base_url: None,
        model: None,
        models: HashMap::new(),
        routing: None,
    };
    let config = Config {
        model: Some(model_config),
        ..Default::default()
    };

    let mut agent = Agent::with_config(config);

    // Submit 8 turns (exceeds the threshold for compression which is > 4)
    for _ in 0..8 {
        let _events = agent
            .submit(Op::UserTurn {
                input: "tool:Read path=file.txt".to_string(),
            })
            .await;
    }

    // Manually trigger compression
    let mut events = Vec::new();
    agent.compact(&mut events).await;

    // Verify ContextCompacted event was emitted
    let compact_event = events.iter().find_map(|e| match e {
        Event::ContextCompacted {
            turn_id,
            compressed_turns,
            retained_turns,
            summary_preview,
        } => Some((
            turn_id,
            compressed_turns,
            retained_turns,
            summary_preview.as_str(),
        )),
        _ => None,
    });

    assert!(
        compact_event.is_some(),
        "compact() should emit ContextCompacted event"
    );

    let (turn_id, compressed_turns, retained_turns, summary_preview) = compact_event.unwrap();

    // With 8 turns: 8 - 3 = 5 turns should be compressed
    // (split point is at turn 5, keeping last 3 turns)
    assert_eq!(
        *compressed_turns, 5,
        "5 turns should be compressed (8 total - 3 retained)"
    );
    assert_eq!(*retained_turns, 3, "3 turns should be retained");
    assert!(
        !summary_preview.is_empty(),
        "summary preview should not be empty"
    );

    // turn_id should be 0 for manual compact (not triggered by auto-compact)
    assert_eq!(*turn_id, 0, "manual compact should use turn_id 0");

    let _ = fs::remove_dir_all(ws);
}

/// Verify that when context_window_size is 0, check_and_auto_compact returns early.
/// By default (no config), context_window_size is 0.
/// Even with many turns, auto-compaction should not trigger.
#[tokio::test]
async fn test_context_window_zero_skips_auto_compact() {
    let ws = workspace();
    fs::write(ws.join("file.txt"), "content").unwrap();

    let _cwd_scope = CwdScope::enter(&ws);

    // Create config with echo backend to allow manual compression without real API key
    let model_config = ModelConfig {
        backend: Some("echo".to_string()),
        api_key: None,
        base_url: None,
        model: None,
        models: HashMap::new(),
        routing: None,
    };
    let config = Config {
        model: Some(model_config),
        ..Default::default()
    };

    let mut agent = Agent::with_config(config);

    // Submit many turns (far exceeds the compression threshold)
    // The compression logic requires > 4 turns
    for i in 0..10 {
            let events = agent
                .submit(Op::UserTurn {
                    input: "tool:Read path=file.txt".to_string(),
                })
                .await;

        // Each turn should complete
        assert!(
            matches!(events.last(), Some(Event::TurnCompleted { .. })),
            "turn {} should complete",
            i
        );

        // No ContextCompacted event should be emitted
        // because context_window_size is 0 (default)
        let has_compacted = events
            .iter()
            .any(|e| matches!(e, Event::ContextCompacted { .. }));
        assert!(
            !has_compacted,
            "with context_window_size=0, auto-compact should not trigger even with many turns"
        );
    }

    // Verify that manually calling compact still works
    // (manual compact ignores context_window_size check)
    let mut events = Vec::new();
    agent.compact(&mut events).await;

    // Manual compact should still work
    let has_compacted = events
        .iter()
        .any(|e| matches!(e, Event::ContextCompacted { .. }));
    assert!(
        has_compacted,
        "manual compact() should still work regardless of context_window_size"
    );

    let _ = fs::remove_dir_all(ws);
}

/// Verify that when context_window_size is non-zero, auto-compact checks emit ContextUsageUpdated.
/// With a large enough window, the usage event should appear without triggering compaction.
#[tokio::test]
async fn test_auto_compact_emits_context_usage_updated() {
    let ws = workspace();
    fs::write(ws.join("file.txt"), "content").unwrap();

    let _cwd_scope = CwdScope::enter(&ws);

    let mut models = HashMap::new();
    models.insert(
        "planner".to_string(),
        ProviderModelConfig {
            context_window_size: Some(100_000),
            ..Default::default()
        },
    );

    let model_config = ModelConfig {
        backend: Some("echo".to_string()),
        api_key: None,
        base_url: None,
        model: None,
        models,
        routing: Some(ModelRoutingConfig {
            planner: Some("planner".to_string()),
            final_writer: None,
            tool_reasoning: None,
            fallbacks: HashMap::new(),
        }),
    };
    let config = Config {
        model: Some(model_config),
        ..Default::default()
    };

    let mut agent = Agent::with_config(config);

    let _ = agent
        .submit(Op::UserTurn {
            input: "seed".to_string(),
        })
        .await;

    let events = agent
        .submit(Op::UserTurn {
            input: "hello".to_string(),
        })
        .await;

    let usage_event = events.iter().find_map(|event| match event {
        Event::ContextUsageUpdated {
            turn_id,
            input_tokens,
            context_window_size,
            ratio,
        } => Some((*turn_id, *input_tokens, *context_window_size, *ratio)),
        _ => None,
    });

    assert!(
        usage_event.is_some(),
        "submit() should emit ContextUsageUpdated when context_window_size is non-zero"
    );

    let (turn_id, input_tokens, context_window_size, ratio) = usage_event.unwrap();
    assert!(turn_id > 0, "usage event should reference the active turn");
    assert!(
        input_tokens > 0,
        "usage event should report positive input tokens"
    );
    assert_eq!(
        context_window_size, 100_000,
        "usage event should echo the configured context window"
    );
    assert!(
        ratio < 0.75,
        "large window should keep ratio below compact threshold"
    );

    let has_compacted = events
        .iter()
        .any(|event| matches!(event, Event::ContextCompacted { .. }));
    assert!(
        !has_compacted,
        "large window should not trigger ContextCompacted in this test"
    );

    let _ = fs::remove_dir_all(ws);
}
