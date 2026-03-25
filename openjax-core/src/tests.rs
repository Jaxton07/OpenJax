use anyhow::Result;
use async_trait::async_trait;
use openjax_protocol::{Event, Op};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{Duration, sleep};

use super::{
    Agent, ApprovalHandler, ApprovalRequest, Config, SandboxMode,
    agent::{
        decision::{normalize_model_decision, parse_model_decision},
        prompt::{build_planner_input, summarize_user_input},
        runtime_policy::{
            parse_sandbox_mode, resolve_max_planner_rounds_per_turn_with_lookup,
            resolve_max_tool_calls_per_turn_with_lookup,
        },
        tool_policy::should_abort_on_consecutive_duplicate_skips,
    },
    model::{ModelClient, ModelRequest, ModelResponse, StreamDelta},
};
use crate::tools::context::{FunctionCallOutputBody, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::DecisionKind;
use openjax_policy::store::PolicyStore;

#[derive(Clone)]
struct ScriptedStreamingModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct ScriptedToolBatchModel {
    complete_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct ScriptedToolBatchDependencyModel {
    complete_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct PlannerFallbackModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct DuplicateToolLoopModel;

#[derive(Clone)]
struct ApprovalBlockedBatchModel {
    stream_calls: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct ApprovalCancellationBatchModel;

struct SlowProbeTool {
    marker_path: PathBuf,
}

#[derive(Debug, Default)]
struct RejectApprovalHandler;

impl ScriptedToolBatchModel {
    fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
        }
    }
}

impl ApprovalBlockedBatchModel {
    fn new() -> Self {
        Self {
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    fn stream_call_count(&self) -> usize {
        *self.stream_calls.lock().expect("stream_calls lock")
    }
}

impl ScriptedToolBatchDependencyModel {
    fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
        }
    }
}

impl PlannerFallbackModel {
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
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let text = if *calls == 1 {
            r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_1","tool_name":"list_dir","arguments":{"path":"."}},{"tool_call_id":"call_2","tool_name":"system_load","arguments":{}}]}"#
        } else {
            r#"{"action":"final","message":"batch done"}"#
        };
        if request.stage == super::model::ModelStage::Planner
            && let Some(sender) = delta_sender
        {
            let _ = sender.send(StreamDelta::Text(text.to_string()));
        }
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "scripted-tool-batch"
    }
}

#[async_trait]
impl ModelClient for ScriptedToolBatchDependencyModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let text = if *calls == 1 {
            r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_1","tool_name":"list_dir","arguments":{"path":"."}},{"tool_call_id":"call_2","tool_name":"system_load","arguments":{},"depends_on":["missing_call"]}]}"#
        } else {
            r#"{"action":"final","message":"dependency done"}"#
        };
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let text = if *calls == 1 {
            r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_1","tool_name":"list_dir","arguments":{"path":"."}},{"tool_call_id":"call_2","tool_name":"system_load","arguments":{},"depends_on":["missing_call"]}]}"#
        } else {
            r#"{"action":"final","message":"dependency done"}"#
        };
        if request.stage == super::model::ModelStage::Planner
            && let Some(sender) = delta_sender
        {
            let _ = sender.send(StreamDelta::Text(text.to_string()));
        }
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "scripted-tool-batch-dependency"
    }
}

#[async_trait]
impl ModelClient for PlannerFallbackModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        Ok(ModelResponse {
            text: r#"{"action":"final","message":"fallback final"}"#.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text("{\"action\":\"to".to_string()));
            let _ = sender.send(StreamDelta::Text("ol\"".to_string()));
        }
        Ok(ModelResponse {
            text: "not valid json".to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "planner-fallback"
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
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut stream_calls = self.stream_calls.lock().expect("stream_calls lock");
        *stream_calls += 1;
        if request.stage == super::model::ModelStage::Planner {
            let planner_text = r#"{"action":"final","message":"seed"}"#.to_string();
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text(
                    "{\"action\":\"final\",\"message\":\"se".to_string(),
                ));
                let _ = sender.send(StreamDelta::Text("ed\"}".to_string()));
            }
            return Ok(ModelResponse {
                text: planner_text,
                ..ModelResponse::default()
            });
        }
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text("你".to_string()));
            let _ = sender.send(StreamDelta::Text("好".to_string()));
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

#[async_trait]
impl ModelClient for DuplicateToolLoopModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            text: r#"{"action":"tool","tool":"shell","args":{"cmd":"echo hi"}}"#.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let text = r#"{"action":"tool","tool":"shell","args":{"cmd":"echo hi"}}"#;
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text(text.to_string()));
        }
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "duplicate-tool-loop"
    }
}

#[async_trait]
impl ModelClient for ApprovalBlockedBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            text: r#"{"action":"final","message":"should not be reached"}"#.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        let text = if *calls == 1 {
            r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_1","tool_name":"list_dir","arguments":{"path":"."}},{"tool_call_id":"call_2","tool_name":"system_load","arguments":{}},{"tool_call_id":"call_3","tool_name":"list_dir","arguments":{"path":"."},"depends_on":["call_1"]}]}"#
        } else {
            r#"{"action":"final","message":"should not be reached"}"#
        };
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text(text.to_string()));
        }
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "approval-blocked-batch"
    }
}

#[async_trait]
impl ModelClient for ApprovalCancellationBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            text: r#"{"action":"final","message":"should not be reached"}"#.to_string(),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let text = r#"{"action":"tool_batch","tool_calls":[{"tool_call_id":"call_approve","tool_name":"edit_file_range","arguments":{"file_path":"todo.txt","start_line":"1","end_line":"1","new_text":"x"}},{"tool_call_id":"call_slow","tool_name":"system_load","arguments":{}}]}"#;
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text(text.to_string()));
        }
        Ok(ModelResponse {
            text: text.to_string(),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "approval-cancellation-batch"
    }
}

#[async_trait]
impl ToolHandler for SlowProbeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(
        &self,
        _invocation: crate::tools::context::ToolInvocation,
    ) -> Result<ToolOutput, FunctionCallError> {
        sleep(Duration::from_millis(250)).await;
        fs::write(&self.marker_path, "ran")
            .map_err(|e| FunctionCallError::Internal(format!("marker write failed: {e}")))?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text("slow probe done".to_string()),
            success: Some(true),
        })
    }
}

#[async_trait]
impl ApprovalHandler for RejectApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
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
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
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
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let mut args = HashMap::new();
    args.insert("cmd".to_string(), "echo hi".to_string());

    agent.record_tool_call("shell", &args, true, "old");
    assert!(agent.is_duplicate_tool_call("shell", &args));

    agent.state_epoch = agent.state_epoch.saturating_add(1);
    assert!(!agent.is_duplicate_tool_call("shell", &args));
}

#[test]
fn planner_prompt_contains_apply_patch_verification_rule() {
    let prompt = build_planner_input("update file", &[], &[], 3, "(none)", None);
    assert!(
        prompt.contains("verification already shows the requested content/changes are present")
    );
    assert!(prompt.contains("return final immediately"));
}

#[test]
fn planner_prompt_contains_skills_section() {
    let prompt = build_planner_input("update file", &[], &[], 3, "- name: rust-debug", None);
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
async fn final_action_emits_response_text_delta_before_completion() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
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

    let events = agent
        .submit(Op::UserTurn {
            input: "你好".to_string(),
        })
        .await;

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

    let events = agent
        .submit(Op::UserTurn {
            input: "你好".to_string(),
        })
        .await;

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
async fn planner_stream_parse_failure_falls_back_to_complete_response() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = PlannerFallbackModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);

    let events = agent
        .submit(Op::UserTurn {
            input: "fallback".to_string(),
        })
        .await;

    let final_message = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::ResponseCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default();

    assert_eq!(final_message, "fallback final");
    assert_eq!(model_probe.stream_call_count(), 1);
    assert_eq!(model_probe.complete_call_count(), 1);
}

#[tokio::test]
async fn tool_batch_emits_proposal_and_batch_completed_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
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

    let events = agent
        .submit(Op::UserTurn {
            input: "run dependency batch".to_string(),
        })
        .await;

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
async fn duplicate_tool_skip_and_abort_emit_response_error_events() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    agent.model_client = Box::new(DuplicateToolLoopModel);
    let mut dup_args = HashMap::new();
    dup_args.insert("cmd".to_string(), "echo hi".to_string());
    agent.record_tool_call("shell", &dup_args, true, "ok");

    let events = agent
        .submit(Op::UserTurn {
            input: "trigger duplicate loop".to_string(),
        })
        .await;

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

#[tokio::test]
async fn tool_batch_approval_blocked_stops_followup_scheduling_and_rounds() {
    let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, PathBuf::from("."));
    let model = ApprovalBlockedBatchModel::new();
    let model_probe = model.clone();
    agent.model_client = Box::new(model);
    agent.set_policy_runtime(Some(PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![],
    ))));
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "run approval blocked batch".to_string(),
        })
        .await;

    let mut approval_blocked_errors = 0usize;
    let mut saw_final_response_completed = false;
    let mut saw_call_3_started = false;
    for event in &events {
        match event {
            Event::ResponseError { code, .. } if code == "approval_blocked" => {
                approval_blocked_errors += 1;
            }
            Event::ResponseCompleted { .. } => saw_final_response_completed = true,
            Event::ToolCallStarted { tool_call_id, .. } if tool_call_id == "call_3" => {
                saw_call_3_started = true;
            }
            _ => {}
        }
    }

    assert_eq!(approval_blocked_errors, 1);
    assert!(!saw_final_response_completed);
    assert!(!saw_call_3_started);
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
    agent.set_policy_runtime(Some(PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![],
    ))));
    agent.set_approval_handler(Arc::new(RejectApprovalHandler));

    let events = agent
        .submit(Op::UserTurn {
            input: "trigger batch cancellation".to_string(),
        })
        .await;

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
    let slow_completed = events.iter().find(|evt| {
        matches!(
            evt,
            Event::ToolCallCompleted {
                tool_call_id,
                ..
            } if tool_call_id == "call_slow"
        )
    });
    match slow_completed {
        Some(Event::ToolCallCompleted { ok, output, .. }) => {
            assert!(!ok, "slow tool should be canceled");
            assert!(
                output.contains("canceled by approval decision"),
                "unexpected cancel output: {output}"
            );
        }
        _ => panic!("expected canceled ToolCallCompleted event for call_slow"),
    }

    assert!(
        !marker_path.exists(),
        "slow tool side-effect should not be committed after cancellation"
    );

    let _ = fs::remove_dir_all(workspace);
}
