use std::collections::HashMap;
use std::path::PathBuf;

use openjax_protocol::Event;

use super::support::{DuplicateToolLoopModel, user_turn};
use crate::agent::tool_policy::should_abort_on_consecutive_duplicate_skips;
use crate::{Agent, SandboxMode};

#[test]
fn duplicate_detection_is_turn_local_when_cleared() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "echo hi".to_string());

    agent.record_tool_call("shell", &args, true, "ok");
    assert!(agent.is_duplicate_tool_call("shell", &args));

    agent.recent_tool_calls.clear();
    assert!(!agent.is_duplicate_tool_call("shell", &args));
}

#[test]
fn duplicate_detection_resets_after_mutation_epoch_change() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "echo hi".to_string());

    agent.record_tool_call("shell", &args, true, "old");
    assert!(agent.is_duplicate_tool_call("shell", &args));

    agent.state_epoch = agent.state_epoch.saturating_add(1);
    assert!(!agent.is_duplicate_tool_call("shell", &args));
}

#[test]
fn aborts_after_consecutive_duplicate_skips() {
    assert!(!should_abort_on_consecutive_duplicate_skips(0, 2));
    assert!(!should_abort_on_consecutive_duplicate_skips(1, 2));
    assert!(should_abort_on_consecutive_duplicate_skips(2, 2));
    assert!(should_abort_on_consecutive_duplicate_skips(3, 2));
}

#[tokio::test]
async fn duplicate_tool_skip_and_abort_emit_response_error_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(DuplicateToolLoopModel);
    let mut dup_args = HashMap::new();
    dup_args.insert("cmd".to_string(), "echo hi".to_string());
    agent.record_tool_call("shell", &dup_args, true, "ok");

    let events = agent.submit(user_turn("trigger duplicate loop")).await;

    let mut saw_duplicate_skip = false;
    let mut saw_duplicate_abort = false;
    let mut saw_assistant_message = false;

    for event in &events {
        match event {
            Event::ResponseError { code, .. } if code == "duplicate_tool_call_skipped" => {
                saw_duplicate_skip = true;
            }
            Event::ResponseError { code, .. } if code == "duplicate_tool_call_loop_abort" => {
                saw_duplicate_abort = true;
            }
            Event::AssistantMessage { .. } => {
                saw_assistant_message = true;
            }
            _ => {}
        }
    }

    assert!(saw_duplicate_skip);
    assert!(saw_duplicate_abort);
    assert!(!saw_assistant_message);
}
