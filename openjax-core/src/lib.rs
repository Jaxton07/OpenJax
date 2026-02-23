mod agent;
pub mod approval;
mod config;
mod logger;
mod model;
pub mod tools;

use agent::runtime_policy::{resolve_approval_policy, resolve_sandbox_mode};
pub use approval::{ApprovalHandler, ApprovalRequest, StdinApprovalHandler};
pub use config::AgentConfig;
pub use config::Config;
pub use logger::init_logger;
use openjax_protocol::Event;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info};

pub use model::build_model_client;
pub use model::build_model_client_with_config;
pub use tools::ApprovalPolicy;
pub use tools::SandboxMode;

// Re-export protocol types for external use
pub use openjax_protocol::{AgentSource, AgentStatus, ThreadId};

const MAX_TOOL_CALLS_PER_TURN: usize = 5;
const MAX_PLANNER_ROUNDS_PER_TURN: usize = 10;
const MAX_CONSECUTIVE_DUPLICATE_SKIPS: usize = 2;
pub(crate) const MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT: usize = 4_000;
const MAX_CONVERSATION_HISTORY_ITEMS: usize = 20;
const USER_INPUT_LOG_PREVIEW_CHARS: usize = 200;

// Rate limiting configuration for API calls
#[derive(Debug, Clone)]
struct RateLimitConfig {
    min_delay_between_requests_ms: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            min_delay_between_requests_ms: 1000,
        }
    }
}

// Retry configuration for tool calls
#[derive(Debug, Clone)]
struct RetryConfig {
    max_retries: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay_ms: 500,
            max_delay_ms: 5000,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HistoryEntry {
    pub(crate) role: &'static str,
    pub(crate) content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ToolCallKey {
    name: String,
    args: String,
}

#[derive(Debug, Clone)]
struct ToolCallRecord {
    key: ToolCallKey,
    ok: bool,
    epoch: u64,
    _output: String,
}

pub struct Agent {
    next_turn_id: u64,
    model_client: Box<dyn model::ModelClient>,
    tools: tools::ToolRouter,
    tool_runtime_config: tools::ToolRuntimeConfig,
    cwd: PathBuf,
    history: Vec<HistoryEntry>,
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    depth: i32,
    last_api_call_time: Option<std::time::Instant>,
    rate_limit_config: RateLimitConfig,
    recent_tool_calls: Vec<ToolCallRecord>,
    state_epoch: u64,
    approval_handler: Arc<dyn approval::ApprovalHandler>,
    event_sink: Option<UnboundedSender<Event>>,
}

impl Agent {
    pub fn new() -> Self {
        let config = Config::load();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::with_config_and_runtime(
            config,
            tools::ApprovalPolicy::from_env(),
            tools::SandboxMode::from_env(),
            cwd,
        )
    }

    pub fn with_config(config: Config) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let approval_policy = resolve_approval_policy(&config);
        let sandbox_mode = resolve_sandbox_mode(&config);
        Self::with_config_and_runtime(config, approval_policy, sandbox_mode, cwd)
    }

    pub fn with_runtime(
        approval_policy: tools::ApprovalPolicy,
        sandbox_mode: tools::SandboxMode,
        cwd: PathBuf,
    ) -> Self {
        let config = Config::load();
        Self::with_config_and_runtime(config, approval_policy, sandbox_mode, cwd)
    }

    pub fn with_config_and_runtime(
        config: Config,
        approval_policy: tools::ApprovalPolicy,
        sandbox_mode: tools::SandboxMode,
        cwd: PathBuf,
    ) -> Self {
        let model_client = model::build_model_client_with_config(config.model.as_ref());
        let thread_id = ThreadId::new();
        info!(
            thread_id = ?thread_id,
            model_backend = model_client.name(),
            approval_policy = approval_policy.as_str(),
            sandbox_mode = sandbox_mode.as_str(),
            cwd = %cwd.display(),
            "agent created"
        );
        Self {
            next_turn_id: 1,
            model_client,
            tools: tools::ToolRouter::with_runtime_config(tools::ToolRuntimeConfig {
                approval_policy,
                sandbox_mode,
                shell_type: tools::ShellType::default(),
                tools_config: tools::spec::ToolsConfig::default(),
            }),
            tool_runtime_config: tools::ToolRuntimeConfig {
                approval_policy,
                sandbox_mode,
                shell_type: tools::ShellType::default(),
                tools_config: tools::spec::ToolsConfig::default(),
            },
            cwd,
            history: Vec::new(),
            thread_id,
            parent_thread_id: None,
            depth: 0,
            last_api_call_time: None,
            rate_limit_config: RateLimitConfig::default(),
            recent_tool_calls: Vec::new(),
            state_epoch: 0,
            approval_handler: Arc::new(approval::StdinApprovalHandler::new()),
            event_sink: None,
        }
    }

    pub fn model_backend_name(&self) -> &'static str {
        self.model_client.name()
    }

    pub fn approval_policy_name(&self) -> &'static str {
        self.tool_runtime_config.approval_policy.as_str()
    }

    pub fn sandbox_mode_name(&self) -> &'static str {
        self.tool_runtime_config.sandbox_mode.as_str()
    }

    pub fn set_approval_handler(&mut self, handler: Arc<dyn approval::ApprovalHandler>) {
        self.approval_handler = handler;
    }

    // ============== Rate Limiting ==============

    async fn apply_rate_limit(&mut self) {
        if let Some(last_time) = self.last_api_call_time {
            let elapsed = last_time.elapsed();
            let min_delay = std::time::Duration::from_millis(
                self.rate_limit_config.min_delay_between_requests_ms,
            );

            if elapsed < min_delay {
                let delay = min_delay - elapsed;
                debug!(
                    delay_ms = delay.as_millis(),
                    "rate_limit: delaying API call"
                );
                tokio::time::sleep(delay).await;
            }
        }
        self.last_api_call_time = Some(std::time::Instant::now());
    }

    // ============== Duplicate Detection ==============

    fn is_duplicate_tool_call(
        &self,
        tool_name: &str,
        args: &std::collections::HashMap<String, String>,
    ) -> bool {
        let key = ToolCallKey {
            name: tool_name.to_string(),
            args: serde_json::to_string(args).unwrap_or_default(),
        };

        self.recent_tool_calls
            .iter()
            .any(|record| record.key == key && record.ok && record.epoch == self.state_epoch)
    }

    fn record_tool_call(
        &mut self,
        tool_name: &str,
        args: &std::collections::HashMap<String, String>,
        ok: bool,
        output: &str,
    ) {
        let key = ToolCallKey {
            name: tool_name.to_string(),
            args: serde_json::to_string(args).unwrap_or_default(),
        };

        self.recent_tool_calls.push(ToolCallRecord {
            key,
            ok,
            epoch: self.state_epoch,
            _output: output.to_string(),
        });

        if self.recent_tool_calls.len() > 10 {
            self.recent_tool_calls.remove(0);
        }
    }

    // ============== Multi-Agent Methods (预留扩展) ==============

    /// Get current agent's thread ID
    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    /// Get parent thread ID (None for root agent)
    pub fn parent_thread_id(&self) -> Option<ThreadId> {
        self.parent_thread_id
    }

    /// Get agent depth in the hierarchy
    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// Check if this agent can spawn sub-agents
    pub fn can_spawn_sub_agent(&self) -> bool {
        self.depth < tools::MAX_AGENT_DEPTH
    }

    /// Create a new sub-agent (预留扩展，未完全实现)
    /// Returns a new Agent instance with incremented depth
    pub fn spawn_sub_agent(&self, _input: &str) -> Result<Agent, String> {
        if !self.can_spawn_sub_agent() {
            return Err(format!(
                "cannot spawn sub-agent: max depth {} reached",
                tools::MAX_AGENT_DEPTH
            ));
        }

        let mut sub_agent = Agent::with_runtime(
            self.tool_runtime_config.approval_policy,
            self.tool_runtime_config.sandbox_mode,
            self.cwd.clone(),
        );

        // Set parent relationship
        sub_agent.parent_thread_id = Some(self.thread_id);
        sub_agent.depth = self.depth + 1;
        sub_agent.approval_handler = self.approval_handler.clone();

        Ok(sub_agent)
    }

    fn record_history(&mut self, role: &'static str, content: String) {
        self.history.push(HistoryEntry { role, content });
        if self.history.len() > MAX_CONVERSATION_HISTORY_ITEMS {
            let overflow = self.history.len() - MAX_CONVERSATION_HISTORY_ITEMS;
            self.history.drain(0..overflow);
        }
    }

    fn push_event(&self, events: &mut Vec<Event>, event: Event) {
        if let Some(sink) = &self.event_sink {
            let _ = sink.send(event.clone());
        }
        events.push(event);
    }
}

pub(crate) fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "apply_patch" | "edit_file_range" | "shell" | "exec_command"
    )
}

pub(crate) fn should_abort_on_consecutive_duplicate_skips(count: usize) -> bool {
    count >= MAX_CONSECUTIVE_DUPLICATE_SKIPS
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_trait::async_trait;
    use openjax_protocol::{Event, Op};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use tokio::sync::mpsc::UnboundedSender;

    use super::{
        Agent, ApprovalPolicy, SandboxMode,
        agent::{
            decision::{normalize_model_decision, parse_model_decision},
            prompt::{build_planner_input, summarize_user_input},
            runtime_policy::{parse_approval_policy, parse_sandbox_mode},
        },
        model::ModelClient,
        should_abort_on_consecutive_duplicate_skips,
    };

    struct ScriptedStreamingModel {
        complete_calls: Mutex<usize>,
    }

    impl ScriptedStreamingModel {
        fn new() -> Self {
            Self {
                complete_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl ModelClient for ScriptedStreamingModel {
        async fn complete(&self, _user_input: &str) -> Result<String> {
            let mut calls = self.complete_calls.lock().expect("complete_calls lock");
            *calls += 1;
            Ok(r#"{"action":"final","message":"seed"}"#.to_string())
        }

        async fn complete_stream(
            &self,
            _user_input: &str,
            delta_sender: Option<UnboundedSender<String>>,
        ) -> Result<String> {
            if let Some(sender) = delta_sender {
                let _ = sender.send("你".to_string());
                let _ = sender.send("好".to_string());
            }
            Ok("你好".to_string())
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
        args.insert("path".to_string(), "test.txt".to_string());

        agent.record_tool_call("read_file", &args, true, "ok");
        assert!(agent.is_duplicate_tool_call("read_file", &args));

        agent.recent_tool_calls.clear();
        assert!(!agent.is_duplicate_tool_call("read_file", &args));
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
    fn duplicate_detection_resets_after_mutation_epoch_change() {
        let mut agent = Agent::with_runtime(
            ApprovalPolicy::Never,
            SandboxMode::WorkspaceWrite,
            PathBuf::from("."),
        );
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());

        agent.record_tool_call("read_file", &args, true, "old");
        assert!(agent.is_duplicate_tool_call("read_file", &args));

        agent.state_epoch = agent.state_epoch.saturating_add(1);
        assert!(!agent.is_duplicate_tool_call("read_file", &args));
    }

    #[test]
    fn planner_prompt_contains_apply_patch_verification_rule() {
        let prompt = build_planner_input("update file", &[], &[], 3);
        assert!(prompt.contains("After a successful apply_patch"));
        assert!(prompt.contains("return final immediately"));
    }

    #[test]
    fn aborts_after_consecutive_duplicate_skips() {
        assert!(!should_abort_on_consecutive_duplicate_skips(0));
        assert!(!should_abort_on_consecutive_duplicate_skips(1));
        assert!(should_abort_on_consecutive_duplicate_skips(2));
        assert!(should_abort_on_consecutive_duplicate_skips(3));
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
        agent.model_client = Box::new(ScriptedStreamingModel::new());

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
    }
}
