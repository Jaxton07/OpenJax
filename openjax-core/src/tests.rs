use anyhow::Result;
use async_trait::async_trait;
use openjax_protocol::{Event, Op};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Agent, ApprovalPolicy, Config, FinalResponseMode, SandboxMode,
    agent::{
        decision::{normalize_model_decision, parse_model_decision},
        prompt::{build_planner_input, summarize_user_input},
        runtime_policy::{
            parse_approval_policy, parse_sandbox_mode,
            resolve_max_planner_rounds_per_turn_with_lookup,
            resolve_max_tool_calls_per_turn_with_lookup,
        },
        tool_policy::should_abort_on_consecutive_duplicate_skips,
    },
    model::{ModelClient, ModelRequest, ModelResponse},
};

#[derive(Clone)]
struct ScriptedStreamingModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct ScriptedToolBatchModel {
    complete_calls: Arc<Mutex<usize>>,
}

impl ScriptedToolBatchModel {
    fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedToolBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let text = if *calls == 1 {
            r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_1","tool_name":"list_dir","arguments":{"path":"."}},{"tool_call_id":"call_2","tool_name":"system_load","arguments":{}}]}"#
        } else {
            r#"{"action":"final","message":"batch done"}"#
        };
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        _delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        Ok(ModelResponse::default())
    }

    fn name(&self) -> &'static str {
        "scripted-tool-batch"
    }
}

impl ScriptedStreamingModel {
    fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    fn complete_call_count(&self) -> usize {
        *self.complete_calls.lock().expect("complete_calls lock")
    }

    fn stream_call_count(&self) -> usize {
        *self.stream_calls.lock().expect("stream_calls lock")
    }
}

#[async_trait]
impl ModelClient for ScriptedStreamingModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        Ok(ModelResponse {
            text: r#"{"action":"final","message":"seed"}"#.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        let mut stream_calls = self.stream_calls.lock().expect("stream_calls lock");
        *stream_calls += 1;
        if let Some(sender) = delta_sender {
            let _ = sender.send("你".to_string());
            let _ = sender.send("好".to_string());
        }
        Ok(ModelResponse {
            text: "你好".to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "scripted-stream"
    }
}

#[test]
fn normalizes_tool_name_in_action_with_top_level_args() {
    let raw = r#"{"action":"read_file","path":"test.txt"}"#;
    let parsed = parse_model_decision(raw).expect("parse decision");
    let decision = normalize_model_decision(parsed);

    assert_eq!(decision.action, "tool");
    assert_eq!(decision.tool.as_deref(), Some("read_file"));
    assert_eq!(
        decision
            .args
            .as_ref()
            .and_then(|m| m.get("path"))
            .map(String::as_str),
        Some("test.txt")
    );
}

#[test]
fn keeps_explicit_tool_shape_unchanged() {
    let raw = r#"{"action":"tool","tool":"apply_patch","args":{"patch":"*** Begin Patch\n*** End Patch"}}"#;
    let parsed = parse_model_decision(raw).expect("parse decision");
    let decision = normalize_model_decision(parsed);

    assert_eq!(decision.action, "tool");
    assert_eq!(decision.tool.as_deref(), Some("apply_patch"));
    assert!(
        decision
            .args
            .as_ref()
            .is_some_and(|m| m.contains_key("patch"))
    );
}

#[test]
fn keeps_final_action_unchanged() {
    let raw = r#"{"action":"final","message":"done"}"#;
    let parsed = parse_model_decision(raw).expect("parse decision");
    let decision = normalize_model_decision(parsed);

    assert_eq!(decision.action, "final");
    assert_eq!(decision.message.as_deref(), Some("done"));
}

#[test]
fn duplicate_detection_is_turn_local_when_cleared() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "echo hi".to_string());

    agent.record_tool_call("shell", &args, true, "ok");
    assert!(agent.is_duplicate_tool_call("shell", &args));

    agent.recent_tool_calls.clear();
    assert!(!agent.is_duplicate_tool_call("shell", &args));
}

#[test]
fn parse_runtime_policies() {
    assert!(matches!(
        parse_approval_policy("always_ask"),
        Some(ApprovalPolicy::AlwaysAsk)
    ));
    assert!(matches!(
        parse_approval_policy("on_request"),
        Some(ApprovalPolicy::OnRequest)
    ));
    assert!(matches!(
        parse_approval_policy("never"),
        Some(ApprovalPolicy::Never)
    ));
    assert!(parse_approval_policy("invalid").is_none());

    assert!(matches!(
        parse_sandbox_mode("workspace_write"),
        Some(SandboxMode::WorkspaceWrite)
    ));
    assert!(matches!(
        parse_sandbox_mode("danger_full_access"),
        Some(SandboxMode::DangerFullAccess)
    ));
    assert!(parse_sandbox_mode("invalid").is_none());
}

#[test]
fn resolves_turn_limits_from_config_and_env_with_precedence() {
    let config = Config {
        agent: Some(crate::AgentConfig {
            max_agents: None,
            max_depth: None,
            max_tool_calls_per_turn: Some(15),
            max_planner_rounds_per_turn: Some(30),
        }),
        ..Config::default()
    };

    assert_eq!(
        resolve_max_tool_calls_per_turn_with_lookup(&config, |_| None),
        15
    );
    assert_eq!(
        resolve_max_planner_rounds_per_turn_with_lookup(&config, |_| None),
        30
    );

    let env_lookup = |key: &str| match key {
        "OPENJAX_MAX_TOOL_CALLS_PER_TURN" => Some("12".to_string()),
        "OPENJAX_MAX_PLANNER_ROUNDS_PER_TURN" => Some("25".to_string()),
        _ => None,
    };
    assert_eq!(
        resolve_max_tool_calls_per_turn_with_lookup(&config, env_lookup),
        12
    );
    assert_eq!(
        resolve_max_planner_rounds_per_turn_with_lookup(&config, env_lookup),
        25
    );
}

#[test]
fn duplicate_detection_resets_after_mutation_epoch_change() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "echo hi".to_string());

    agent.record_tool_call("shell", &args, true, "old");
    assert!(agent.is_duplicate_tool_call("shell", &args));

    agent.state_epoch = agent.state_epoch.saturating_add(1);
    assert!(!agent.is_duplicate_tool_call("shell", &args));
}

#[test]
fn planner_prompt_contains_apply_patch_verification_rule() {
    let prompt = build_planner_input("update file", &[], &[], 3, "(none)");
    assert!(prompt.contains("After a successful apply_patch"));
    assert!(prompt.contains("return final immediately"));
}

#[test]
fn planner_prompt_contains_skills_section() {
    let prompt = build_planner_input("update file", &[], &[], 3, "- name: rust-debug");
    assert!(prompt.contains("Available skills (auto-selected):"));
    assert!(prompt.contains("- name: rust-debug"));
}

#[test]
fn aborts_after_consecutive_duplicate_skips() {
    assert!(!should_abort_on_consecutive_duplicate_skips(0, 2));
    assert!(!should_abort_on_consecutive_duplicate_skips(1, 2));
    assert!(should_abort_on_consecutive_duplicate_skips(2, 2));
    assert!(should_abort_on_consecutive_duplicate_skips(3, 2));
}

#[test]
fn summarize_user_input_escapes_control_newlines() {
    let (preview, truncated) = summarize_user_input("hello\nworld", 40);
    assert_eq!(preview, "hello\\nworld");
    assert!(!truncated);
}

#[test]
fn summarize_user_input_adds_ellipsis_when_truncated() {
    let (preview, truncated) = summarize_user_input("abcdef", 3);
    assert_eq!(preview, "abc...");
    assert!(truncated);
}

#[tokio::test]
async fn final_action_emits_assistant_delta_before_message() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    agent.final_response_mode = FinalResponseMode::FinalWriter;
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent
        .submit(Op::UserTurn {
            input: "你好".to_string(),
        })
        .await;

    let mut delta_text = String::new();
    let mut first_delta_index: Option<usize> = None;
    let mut assistant_message_index: Option<usize> = None;
    let mut assistant_message_text = String::new();

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::AssistantDelta { content_delta, .. } => {
                if first_delta_index.is_none() {
                    first_delta_index = Some(idx);
                }
                delta_text.push_str(content_delta);
            }
            Event::AssistantMessage { content, .. } => {
                assistant_message_index = Some(idx);
                assistant_message_text = content.clone();
            }
            _ => {}
        }
    }

    assert_eq!(delta_text, "你好");
    assert_eq!(assistant_message_text, "你好");
    assert!(
        first_delta_index.is_some(),
        "expected assistant delta events"
    );
    assert!(
        assistant_message_index.is_some(),
        "expected final assistant message"
    );
    assert!(
        first_delta_index.expect("first delta")
            < assistant_message_index.expect("assistant message index"),
        "assistant delta should be emitted before final assistant message"
    );
    assert_eq!(model_probe.complete_call_count(), 1);
    assert_eq!(model_probe.stream_call_count(), 1);
}

#[tokio::test]
async fn planner_only_mode_skips_final_writer_and_keeps_delta_events() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    agent.final_response_mode = FinalResponseMode::PlannerOnly;
    agent.stream_engine_v2_enabled = false;
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent
        .submit(Op::UserTurn {
            input: "你好".to_string(),
        })
        .await;

    let mut delta_text = String::new();
    let mut first_delta_index: Option<usize> = None;
    let mut assistant_message_index: Option<usize> = None;
    let mut assistant_message_text = String::new();

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::AssistantDelta { content_delta, .. } => {
                if first_delta_index.is_none() {
                    first_delta_index = Some(idx);
                }
                delta_text.push_str(content_delta);
            }
            Event::AssistantMessage { content, .. } => {
                assistant_message_index = Some(idx);
                assistant_message_text = content.clone();
            }
            _ => {}
        }
    }

    assert_eq!(delta_text, "seed");
    assert_eq!(assistant_message_text, "seed");
    assert!(
        first_delta_index.expect("first delta")
            < assistant_message_index.expect("assistant message index"),
        "assistant delta should be emitted before final assistant message"
    );
    assert_eq!(model_probe.complete_call_count(), 1);
    assert_eq!(model_probe.stream_call_count(), 0);
}

#[tokio::test]
async fn planner_only_mode_with_stream_engine_v2_still_skips_final_writer() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    agent.final_response_mode = FinalResponseMode::PlannerOnly;
    agent.stream_engine_v2_enabled = true;
    let model = ScriptedStreamingModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent
        .submit(Op::UserTurn {
            input: "你好".to_string(),
        })
        .await;

    let mut saw_response_started = false;
    let mut saw_response_completed = false;
    let mut assistant_message_text = String::new();

    for event in &events {
        match event {
            Event::ResponseStarted { .. } => saw_response_started = true,
            Event::ResponseCompleted { content, .. } => {
                saw_response_completed = true;
                assert_eq!(content, "seed");
            }
            Event::AssistantMessage { content, .. } => {
                assistant_message_text = content.clone();
            }
            _ => {}
        }
    }

    assert!(saw_response_started);
    assert!(saw_response_completed);
    assert_eq!(assistant_message_text, "seed");
    assert_eq!(model_probe.complete_call_count(), 1);
    assert_eq!(model_probe.stream_call_count(), 0);
}

#[tokio::test]
async fn tool_batch_emits_proposal_and_batch_completed_events() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );
    agent.final_response_mode = FinalResponseMode::PlannerOnly;
    agent.stream_engine_v2_enabled = false;
    agent.model_client = Box::new(ScriptedToolBatchModel::new());

    let events = agent
        .submit(Op::UserTurn {
            input: "run batch".to_string(),
        })
        .await;

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
            Event::AssistantMessage { content, .. } => {
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
