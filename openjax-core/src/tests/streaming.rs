use std::path::PathBuf;

use openjax_protocol::Event;

use super::support::{
    MixedTextToolUseModel, NativeStreamingFinalModel, NativeStreamingToolUseModel,
    ScriptedStreamingModel, user_turn,
};
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
