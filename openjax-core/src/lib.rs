mod agent;
pub mod approval;
mod config;
mod logger;
mod model;
pub mod tools;

use agent::decision::{normalize_model_decision, parse_model_decision};
use agent::prompt::{
    build_final_response_prompt, build_json_repair_prompt, build_planner_input, truncate_for_prompt,
};
use agent::runtime_policy::{resolve_approval_policy, resolve_sandbox_mode};
pub use approval::{ApprovalHandler, ApprovalRequest, StdinApprovalHandler};
pub use config::AgentConfig;
pub use config::Config;
pub use logger::init_logger;
use openjax_protocol::Event;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

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

    async fn execute_natural_language_turn(
        &mut self,
        turn_id: u64,
        user_input: &str,
        events: &mut Vec<Event>,
    ) {
        let mut tool_traces: Vec<String> = Vec::new();
        let mut executed_count = 0usize;
        let mut planner_rounds = 0usize;
        let mut consecutive_duplicate_skips = 0usize;

        debug!(
            turn_id = turn_id,
            user_input_len = user_input.len(),
            "natural_language_turn started"
        );

        while executed_count < MAX_TOOL_CALLS_PER_TURN
            && planner_rounds < MAX_PLANNER_ROUNDS_PER_TURN
        {
            let remaining = MAX_TOOL_CALLS_PER_TURN - executed_count;
            let planner_input =
                build_planner_input(user_input, &self.history, &tool_traces, remaining);

            info!(
                turn_id = turn_id,
                phase = "thinking",
                planner_round = planner_rounds + 1,
                "llm_state"
            );

            self.apply_rate_limit().await;

            info!(
                turn_id = turn_id,
                executed_count = executed_count,
                remaining_calls = remaining,
                prompt_len = planner_input.len(),
                "model_request started"
            );
            planner_rounds += 1;

            let model_output = match self.model_client.complete(&planner_input).await {
                Ok(output) => output,
                Err(err) => {
                    warn!(
                        turn_id = turn_id,
                        phase = "failed",
                        error = %err,
                        "model_request failed"
                    );
                    let message = format!("[model error] {err}");
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        },
                    );
                    self.record_history("assistant", message);
                    return;
                }
            };

            info!(
                turn_id = turn_id,
                output_len = model_output.len(),
                "model_request completed"
            );

            debug!(
                turn_id = turn_id,
                raw_output = %model_output,
                "model_raw_output"
            );

            let decision = if let Some(parsed) = parse_model_decision(&model_output) {
                normalize_model_decision(parsed)
            } else {
                info!(
                    turn_id = turn_id,
                    "model_output not valid JSON, attempting repair"
                );
                let repair_prompt = build_json_repair_prompt(&model_output);

                self.apply_rate_limit().await;

                info!(turn_id = turn_id, "model_repair_request started");

                let repaired_output = match self.model_client.complete(&repair_prompt).await {
                    Ok(output) => output,
                    Err(err) => {
                        warn!(
                            turn_id = turn_id,
                            error = %err,
                            "model_repair_request failed"
                        );
                        let message = format!("[model error] {err}");
                        self.push_event(
                            events,
                            Event::AssistantMessage {
                                turn_id,
                                content: message.clone(),
                            },
                        );
                        self.record_history("assistant", message);
                        return;
                    }
                };

                info!(turn_id = turn_id, "model_repair_request completed");

                debug!(
                    turn_id = turn_id,
                    repaired_output = %repaired_output,
                    "model_repaired_output"
                );

                if let Some(parsed) = parse_model_decision(&repaired_output) {
                    normalize_model_decision(parsed)
                } else {
                    // Still non-JSON after one repair attempt: treat first output as final.
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: model_output.clone(),
                        },
                    );
                    self.record_history("assistant", model_output);
                    return;
                }
            };

            let action = decision.action.to_ascii_lowercase();

            info!(
                turn_id = turn_id,
                action = %action,
                tool = ?decision.tool,
                args = ?decision.args,
                message = ?decision.message,
                "model_decision"
            );

            if action == "final" {
                let seed_message = decision
                    .message
                    .unwrap_or_else(|| "任务已完成。".to_string());
                info!(
                    turn_id = turn_id,
                    phase = "completed",
                    action = "final",
                    "natural_language_turn completed"
                );
                let message = self
                    .stream_final_assistant_reply(
                        turn_id,
                        user_input,
                        &tool_traces,
                        &seed_message,
                        events,
                    )
                    .await;
                self.push_event(
                    events,
                    Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    },
                );
                self.record_history("assistant", message);
                return;
            }

            if action == "tool" {
                let tool_name = match decision.tool {
                    Some(name) if !name.trim().is_empty() => name,
                    _ => {
                        warn!(turn_id = turn_id, "model_decision missing tool name");
                        let message = "[model error] tool action missing tool name".to_string();
                        self.push_event(
                            events,
                            Event::AssistantMessage {
                                turn_id,
                                content: message.clone(),
                            },
                        );
                        self.record_history("assistant", message);
                        return;
                    }
                };

                let args = decision.args.clone().unwrap_or_default();

                if self.is_duplicate_tool_call(&tool_name, &args) {
                    warn!(
                        turn_id = turn_id,
                        tool_name = %tool_name,
                        args = ?args,
                        "duplicate_tool_call detected, skipping"
                    );
                    let message = format!(
                        "[warning] tool {} with args {:?} was already called recently, skipping",
                        tool_name, args
                    );
                    self.push_event(
                        events,
                        Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        },
                    );
                    self.record_history("assistant", message);
                    tool_traces.push(format!(
                        "tool={tool_name}; ok=skipped_duplicate; args={}",
                        serde_json::to_string(&args).unwrap_or_default()
                    ));
                    consecutive_duplicate_skips = consecutive_duplicate_skips.saturating_add(1);
                    if should_abort_on_consecutive_duplicate_skips(consecutive_duplicate_skips) {
                        let loop_message = format!(
                            "检测到连续 {} 次重复工具调用，已提前结束本回合以避免循环。请继续下一轮或换一种指令。",
                            MAX_CONSECUTIVE_DUPLICATE_SKIPS
                        );
                        self.push_event(
                            events,
                            Event::AssistantMessage {
                                turn_id,
                                content: loop_message.clone(),
                            },
                        );
                        self.record_history("assistant", loop_message);
                        return;
                    }
                    continue;
                }

                let call = tools::ToolCall {
                    name: tool_name.clone(),
                    args: args.clone(),
                };

                let start_time = Instant::now();
                info!(
                    turn_id = turn_id,
                    tool_name = %call.name,
                    args = ?call.args,
                    "tool_call started"
                );

                self.push_event(
                    events,
                    Event::ToolCallStarted {
                        turn_id,
                        tool_name: tool_name.clone(),
                    },
                );

                match self
                    .execute_tool_with_live_events(turn_id, &call, events)
                    .await
                {
                    Ok(output) => {
                        if is_mutating_tool(&tool_name) {
                            // File/content state has changed. Move to a new epoch so read/list
                            // calls with the same args can run again against fresh state.
                            self.state_epoch = self.state_epoch.saturating_add(1);
                        }

                        let duration_ms = start_time.elapsed().as_millis();
                        info!(
                            turn_id = turn_id,
                            tool_name = %tool_name,
                            ok = true,
                            duration_ms = duration_ms,
                            output_len = output.len(),
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok=true; output={}",
                            truncate_for_prompt(&output)
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, true, &output);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                ok: true,
                                output: output.to_string(),
                            },
                        );
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                    }
                    Err(err) => {
                        let duration_ms = start_time.elapsed().as_millis();
                        let err_text = err.to_string();
                        info!(
                            turn_id = turn_id,
                            tool_name = %tool_name,
                            ok = false,
                            duration_ms = duration_ms,
                            error = %err_text,
                            "tool_call completed"
                        );
                        let trace = format!(
                            "tool={tool_name}; ok=false; output={}",
                            truncate_for_prompt(&err_text)
                        );
                        tool_traces.push(trace);

                        self.record_tool_call(&tool_name, &args, false, &err_text);

                        self.push_event(
                            events,
                            Event::ToolCallCompleted {
                                turn_id,
                                tool_name: tool_name.to_string(),
                                ok: false,
                                output: err_text.to_string(),
                            },
                        );
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                        if err_text.to_lowercase().contains("approval rejected") {
                            let stop_message =
                                "操作已取消：用户拒绝了工具调用，本回合已停止。".to_string();
                            self.push_event(
                                events,
                                Event::AssistantMessage {
                                    turn_id,
                                    content: stop_message.clone(),
                                },
                            );
                            self.record_history("assistant", stop_message);
                            return;
                        }
                    }
                }

                continue;
            }

            warn!(
                turn_id = turn_id,
                action = %decision.action,
                "model_decision unsupported action"
            );
            let message = format!("[model error] unsupported action: {}", decision.action);
            self.push_event(
                events,
                Event::AssistantMessage {
                    turn_id,
                    content: message.clone(),
                },
            );
            self.record_history("assistant", message);
            return;
        }

        let message = if executed_count >= MAX_TOOL_CALLS_PER_TURN {
            warn!(
                turn_id = turn_id,
                max_calls = MAX_TOOL_CALLS_PER_TURN,
                "natural_language_turn reached max tool calls"
            );
            format!(
                "已达到单回合最多 {} 次工具调用限制，请继续下一轮。",
                MAX_TOOL_CALLS_PER_TURN
            )
        } else {
            warn!(
                turn_id = turn_id,
                planner_rounds = planner_rounds,
                max_rounds = MAX_PLANNER_ROUNDS_PER_TURN,
                "natural_language_turn reached max planner rounds"
            );
            format!(
                "已达到单回合最多 {} 次规划轮次限制，请继续下一轮。",
                MAX_PLANNER_ROUNDS_PER_TURN
            )
        };
        self.push_event(
            events,
            Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            },
        );
        self.record_history("assistant", message);
    }

    fn record_history(&mut self, role: &'static str, content: String) {
        self.history.push(HistoryEntry { role, content });
        if self.history.len() > MAX_CONVERSATION_HISTORY_ITEMS {
            let overflow = self.history.len() - MAX_CONVERSATION_HISTORY_ITEMS;
            self.history.drain(0..overflow);
        }
    }

    async fn stream_final_assistant_reply(
        &mut self,
        turn_id: u64,
        user_input: &str,
        tool_traces: &[String],
        seed_message: &str,
        events: &mut Vec<Event>,
    ) -> String {
        let prompt = build_final_response_prompt(user_input, tool_traces, seed_message);
        self.apply_rate_limit().await;

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel();
        let stream_future = self.model_client.complete_stream(&prompt, Some(delta_tx));
        tokio::pin!(stream_future);

        let mut streamed = String::new();
        let result = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        if !delta.is_empty() {
                            streamed.push_str(&delta);
                            self.push_event(events, Event::AssistantDelta {
                                turn_id,
                                content_delta: delta,
                            });
                        }
                    }
                }
                result = &mut stream_future => {
                    break result;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            if !delta.is_empty() {
                streamed.push_str(&delta);
                self.push_event(
                    events,
                    Event::AssistantDelta {
                        turn_id,
                        content_delta: delta,
                    },
                );
            }
        }

        match result {
            Ok(full_text) => {
                if streamed.is_empty() {
                    return full_text;
                }
                if full_text.is_empty() {
                    return streamed;
                }
                if full_text == streamed {
                    return streamed;
                }
                full_text
            }
            Err(err) => {
                warn!(turn_id = turn_id, error = %err, "final response streaming failed; fallback to planner message");
                seed_message.to_string()
            }
        }
    }

    fn push_event(&self, events: &mut Vec<Event>, event: Event) {
        if let Some(sink) = &self.event_sink {
            let _ = sink.send(event.clone());
        }
        events.push(event);
    }
}

fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "apply_patch" | "edit_file_range" | "shell" | "exec_command"
    )
}

fn should_abort_on_consecutive_duplicate_skips(count: usize) -> bool {
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
