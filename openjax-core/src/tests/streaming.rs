use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use openjax_protocol::Event;

use super::support::{
    MixedTextToolUseModel, NativeStreamingFinalModel, NativeStreamingToolUseModel,
    ReasoningHistoryProbeModel, RejectApprovalHandler, ScriptedStreamingModel,
    ShellToolResultEchoModel, user_turn,
};
use crate::model::{AssistantContentBlock, ConversationMessage};
use crate::tools::{ToolCall, ToolExecutionRequest, ToolRouter, ToolRuntimeConfig};
use crate::{Agent, SandboxMode};

#[tokio::test]
async fn final_action_emits_response_text_delta_before_completion() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("你好")).await;

    let mut delta_text = String::new();
    let mut first_delta_index: Option<usize> = None;
    let mut response_completed_index: Option<usize> = None;
    let mut completed_text = String::new();

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::ResponseTextDelta { content_delta, .. } => {
                if first_delta_index.is_none() {
                    first_delta_index = Some(idx);
                }
                delta_text.push_str(content_delta);
            }
            Event::ResponseCompleted { content, .. } => {
                response_completed_index = Some(idx);
                completed_text = content.clone();
            }
            _ => {}
        }
    }

    assert_eq!(delta_text, "seed");
    assert_eq!(completed_text, "seed");
    assert!(
        first_delta_index.is_some(),
        "expected response text delta events"
    );
    assert!(
        response_completed_index.is_some(),
        "expected response_completed event"
    );
    assert!(
        first_delta_index.expect("first delta")
            < response_completed_index.expect("response completed index"),
        "response text delta should be emitted before response_completed"
    );
    assert_eq!(model_probe.complete_call_count(), 0);
    assert_eq!(model_probe.stream_call_count(), 1);
}

#[tokio::test]
async fn planner_only_mode_skips_final_writer_and_keeps_response_delta_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("你好")).await;

    let mut delta_text = String::new();
    let mut first_delta_index: Option<usize> = None;
    let mut response_completed_index: Option<usize> = None;
    let mut completed_text = String::new();

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::ResponseTextDelta { content_delta, .. } => {
                if first_delta_index.is_none() {
                    first_delta_index = Some(idx);
                }
                delta_text.push_str(content_delta);
            }
            Event::ResponseCompleted { content, .. } => {
                response_completed_index = Some(idx);
                completed_text = content.clone();
            }
            _ => {}
        }
    }

    assert_eq!(delta_text, "seed");
    assert_eq!(completed_text, "seed");
    assert!(
        first_delta_index.expect("first delta")
            < response_completed_index.expect("response completed index"),
        "response text delta should be emitted before response_completed"
    );
    assert_eq!(model_probe.complete_call_count(), 0);
    assert_eq!(model_probe.stream_call_count(), 1);
}

#[tokio::test]
async fn planner_only_mode_with_stream_engine_v2_still_skips_final_writer() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("你好")).await;

    let mut saw_response_started = false;
    let mut saw_response_completed = false;
    for event in &events {
        match event {
            Event::ResponseStarted { .. } => saw_response_started = true,
            Event::ResponseCompleted { content, .. } => {
                saw_response_completed = true;
                assert_eq!(content, "seed");
            }
            _ => {}
        }
    }

    assert!(saw_response_started);
    assert!(saw_response_completed);
    assert_eq!(model_probe.complete_call_count(), 0);
    assert_eq!(model_probe.stream_call_count(), 1);
}

#[tokio::test]
async fn native_streaming_final_response_does_not_fallback_to_complete() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = NativeStreamingFinalModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("native final")).await;

    let final_message = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::ResponseCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default();

    assert_eq!(final_message, "seed");
    assert_eq!(model_probe.stream_call_count(), 1);
    assert_eq!(model_probe.complete_call_count(), 0);
}

#[tokio::test]
async fn planner_stream_tool_events_preserve_tool_name_across_args_delta_and_ready() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = NativeStreamingToolUseModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("native tool")).await;

    let args_delta_name = events.iter().find_map(|event| match event {
        Event::ToolCallArgsDelta {
            tool_call_id,
            tool_name,
            ..
        } if tool_call_id == "call_1" => Some(tool_name.clone()),
        _ => None,
    });
    let ready_name = events.iter().find_map(|event| match event {
        Event::ToolCallReady {
            tool_call_id,
            tool_name,
            ..
        } if tool_call_id == "call_1" => Some(tool_name.clone()),
        _ => None,
    });
    let final_message = events.iter().rev().find_map(|event| match event {
        Event::ResponseCompleted { content, .. } => Some(content.clone()),
        _ => None,
    });

    assert_eq!(args_delta_name.as_deref(), Some("list_dir"));
    assert_eq!(ready_name.as_deref(), Some("list_dir"));
    assert_eq!(final_message.as_deref(), Some("native tool done"));
    assert_eq!(model_probe.complete_call_count(), 0);
}

#[tokio::test]
async fn tool_use_round_does_not_emit_intermediate_response_text_delta() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(MixedTextToolUseModel::new());

    let events = agent.submit(user_turn("mixed text tool use")).await;

    let deltas = events
        .iter()
        .filter_map(|event| match event {
            Event::ResponseTextDelta { content_delta, .. } => Some(content_delta.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(deltas, vec!["done".to_string()]);
}

#[tokio::test]
async fn tool_use_followup_request_preserves_assistant_reasoning_history() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ReasoningHistoryProbeModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent.submit(user_turn("inspect and continue")).await;

    let requests = model_probe.recorded_requests();
    assert_eq!(requests.len(), 2, "expected initial and follow-up model requests");

    let followup_assistant = requests[1]
        .messages
        .iter()
        .find_map(|message| match message {
            ConversationMessage::Assistant(blocks) => Some(blocks),
            _ => None,
        })
        .expect("assistant history in follow-up request");

    assert!(matches!(
        &followup_assistant[0],
        AssistantContentBlock::Reasoning { text }
            if text == "inspect environment before tool call"
    ));
    assert!(matches!(
        &followup_assistant[1],
        AssistantContentBlock::ToolUse { name, .. } if name == "system_load"
    ));

    let final_message = events.iter().rev().find_map(|event| match event {
        Event::ResponseCompleted { content, .. } => Some(content.clone()),
        _ => None,
    });
    assert_eq!(final_message.as_deref(), Some("done"));
}

#[test]
fn tool_exec_outcome_keeps_model_content_separate_from_display_output() {
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "printf split-contract".to_string());
    let call = ToolCall {
        name: "shell".to_string(),
        args,
    };
    let router = ToolRouter::new();
    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    let outcome = runtime
        .block_on(router.execute(ToolExecutionRequest {
            turn_id: 1,
            session_id: None,
            tool_call_id: "contract_split".to_string(),
            call: &call,
            cwd: PathBuf::from(".").as_path(),
            config: ToolRuntimeConfig::default(),
            approval_handler: Arc::new(RejectApprovalHandler),
            event_sink: None,
            policy_runtime: None,
        }))
        .expect("shell execution should succeed");

    assert!(
        !outcome.model_content.contains("result_class=")
            && !outcome.model_content.contains("command="),
        "model-facing content should be clean and not include display metadata"
    );
    assert!(
        outcome.display_output.contains("result_class="),
        "display output should preserve shell metadata"
    );
    assert!(
        outcome.shell_metadata.is_some(),
        "shell execution should expose structured metadata"
    );
    assert_ne!(
        outcome.model_content, outcome.display_output,
        "model and display channels should be split for shell output"
    );
}

#[tokio::test]
async fn native_tool_result_uses_model_content_not_display_output() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(ShellToolResultEchoModel::new());

    let events = agent.submit(user_turn("native shell split")).await;

    let completed_output = events.iter().find_map(|event| match event {
        Event::ToolCallCompleted {
            tool_name, output, ..
        } if tool_name == "shell" => Some(output.clone()),
        _ => None,
    });
    let final_message = events.iter().rev().find_map(|event| match event {
        Event::ResponseCompleted { content, .. } => Some(content.clone()),
        _ => None,
    });

    let completed_output = completed_output.expect("expected shell completed output");
    let final_message = final_message.expect("expected response completed content");
    assert!(
        !final_message.is_empty(),
        "model-visible tool result should not be empty"
    );
    assert!(
        completed_output.contains("result_class="),
        "display output should retain shell execution classification"
    );
    assert!(
        !final_message.contains("result_class=") && !final_message.contains("command="),
        "model-facing content should not include display metadata"
    );
    assert_ne!(
        final_message, completed_output,
        "model content should be separated from event/display output"
    );
}
