use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use openjax_protocol::Event;

use super::support::{
    ApprovalBlockedBatchModel, ApprovalCancellationBatchModel, OverflowToolUseModel,
    RejectApprovalHandler, ScriptedToolBatchDependencyModel, ScriptedToolBatchModel,
    ShellToolResultEchoModel, SlowProbeTool, ask_policy_runtime, user_turn,
};
use crate::{Agent, SandboxMode};

#[tokio::test]
async fn tool_batch_emits_proposal_and_batch_completed_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(ScriptedToolBatchModel::new());

    let events = agent.submit(user_turn("run batch")).await;

    let mut saw_proposal = false;
    let mut saw_batch_completed = false;
    let mut started_calls = 0usize;
    let mut completed_calls = 0usize;
    let mut final_message = String::new();
    for event in &events {
        match event {
            Event::ToolCallsProposed { tool_calls, .. } => {
                saw_proposal = true;
                assert_eq!(tool_calls.len(), 2);
            }
            Event::ToolCallStarted { .. } => started_calls += 1,
            Event::ToolCallCompleted { .. } => completed_calls += 1,
            Event::ToolBatchCompleted { total, .. } => {
                saw_batch_completed = true;
                assert_eq!(*total, 2);
            }
            Event::ResponseCompleted { content, .. } => {
                final_message = content.clone();
            }
            _ => {}
        }
    }

    assert!(saw_proposal);
    assert!(saw_batch_completed);
    assert_eq!(started_calls, 2);
    assert_eq!(completed_calls, 2);
    assert_eq!(final_message, "batch done");
}

#[tokio::test]
async fn tool_batch_dependency_unmet_still_emits_started_before_completed() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(ScriptedToolBatchDependencyModel::new());

    let events = agent.submit(user_turn("run dependency batch")).await;

    let started_idx = events.iter().position(|event| {
        matches!(
            event,
            Event::ToolCallStarted {
                tool_call_id,
                ..
            } if tool_call_id == "call_2"
        )
    });
    let completed_idx = events.iter().position(|event| {
        matches!(
            event,
            Event::ToolCallCompleted {
                tool_call_id,
                ..
            } if tool_call_id == "call_2"
        )
    });
    assert!(
        started_idx.is_some(),
        "expected started event for unresolved call"
    );
    assert!(
        completed_idx.is_some(),
        "expected completed event for unresolved call"
    );
    assert!(started_idx < completed_idx);
}

#[tokio::test]
async fn tool_batch_approval_blocked_stops_followup_scheduling_and_rounds() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ApprovalBlockedBatchModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);
    agent.set_policy_runtime(Some(ask_policy_runtime()));
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent.submit(user_turn("run approval blocked batch")).await;

    let mut approval_blocked_errors = 0usize;
    let mut saw_final_response_completed = false;
    let mut saw_call_3_progress = false;
    let mut saw_call_3_completed = false;
    for event in &events {
        match event {
            Event::ResponseError { code, .. } if code == "approval_blocked" => {
                approval_blocked_errors += 1;
            }
            Event::ResponseCompleted { .. } => saw_final_response_completed = true,
            Event::ToolCallProgress { tool_call_id, .. } if tool_call_id == "call_3" => {
                saw_call_3_progress = true;
            }
            Event::ToolCallCompleted { tool_call_id, .. } if tool_call_id == "call_3" => {
                saw_call_3_completed = true;
            }
            _ => {}
        }
    }

    assert_eq!(approval_blocked_errors, 1);
    assert!(!saw_final_response_completed);
    assert!(!saw_call_3_progress);
    assert!(!saw_call_3_completed);
    assert_eq!(model_probe.stream_call_count(), 1);
}

#[tokio::test]
async fn tool_batch_approval_blocked_cancels_pending_parallel_tool() {
    let workspace =
        std::env::temp_dir().join(format!("openjax-tool-batch-cancel-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("todo.txt"), "a\nb\n").expect("seed file");

    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, workspace.clone());
    agent.model_client = Box::new(ApprovalCancellationBatchModel);
    let marker_path = workspace.join("slow_probe_marker.txt");
    agent.tools.register_tool(
        "system_load".to_string(),
        Arc::new(SlowProbeTool {
            marker_path: marker_path.clone(),
        }),
    );
    agent.set_policy_runtime(Some(ask_policy_runtime()));
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent.submit(user_turn("trigger batch cancellation")).await;

    assert!(
        events.iter().any(
            |evt| matches!(evt, Event::ResponseError { code, .. } if code == "approval_blocked")
        )
    );
    assert!(
        !events
            .iter()
            .any(|evt| matches!(evt, Event::ResponseCompleted { .. })),
        "turn should not continue to final after approval blocked"
    );
    assert!(
        !events.iter().any(|evt| matches!(
            evt,
            Event::ToolCallProgress { tool_call_id, .. } if tool_call_id == "call_slow"
        )),
        "slow tool should never start executing after approval blocked"
    );
    assert!(
        !events.iter().any(|evt| matches!(
            evt,
            Event::ToolCallCompleted { tool_call_id, .. } if tool_call_id == "call_slow"
        )),
        "slow tool should not emit completed after approval blocked in sequential native execution"
    );

    assert!(
        !marker_path.exists(),
        "slow tool side-effect should not be committed after cancellation"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn overflow_tool_uses_are_closed_with_failed_and_completed_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(OverflowToolUseModel::new());

    let events = agent.submit(user_turn("overflow tool uses")).await;

    let overflow_failed = events.iter().find(|event| {
        matches!(
            event,
            Event::ToolCallFailed {
                tool_call_id,
                code,
                ..
            } if tool_call_id == "overflow_10" && code == "tool_call_budget_exhausted"
        )
    });
    let overflow_completed = events.iter().find(|event| {
        matches!(
            event,
            Event::ToolCallCompleted {
                tool_call_id,
                ok,
                ..
            } if tool_call_id == "overflow_10" && !ok
        )
    });

    assert!(overflow_failed.is_some(), "expected overflow call to fail");
    assert!(
        overflow_completed.is_some(),
        "expected overflow call to emit completed"
    );
}

#[tokio::test]
async fn tool_call_completed_event_contains_structured_shell_metadata() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(ShellToolResultEchoModel::new());

    let events = agent.submit(user_turn("shell metadata event")).await;
    let shell_completed = events
        .iter()
        .find(|event| {
            matches!(
                event,
                Event::ToolCallCompleted { tool_name, .. } if tool_name == "shell"
            )
        })
        .expect("expected shell ToolCallCompleted event");

    let event_json = serde_json::to_value(shell_completed).expect("serialize event to json");
    let metadata = event_json
        .get("ToolCallCompleted")
        .and_then(|payload| payload.get("shell_metadata"));

    assert!(
        matches!(metadata, Some(value) if !value.is_null()),
        "expected shell metadata in ToolCallCompleted, got {event_json}"
    );
}
