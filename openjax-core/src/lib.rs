pub mod approval;
mod config;
mod logger;
mod model;
pub mod tools;

pub use approval::{ApprovalHandler, ApprovalRequest, StdinApprovalHandler};
pub use config::AgentConfig;
pub use config::Config;
pub use logger::init_logger;
use openjax_protocol::{Event, Op};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
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
const MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT: usize = 4_000;
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

#[derive(Debug, Deserialize)]
struct ModelDecision {
    #[serde(alias = "type")]
    action: String,
    tool: Option<String>,
    args: Option<HashMap<String, String>>,
    message: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    role: &'static str,
    content: String,
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

    // ============== Main Submit Method ==============

    pub async fn submit_with_sink(
        &mut self,
        op: Op,
        sink: UnboundedSender<Event>,
    ) -> Vec<Event> {
        self.event_sink = Some(sink);
        let events = self.submit(op).await;
        self.event_sink = None;
        events
    }

    pub async fn submit(&mut self, op: Op) -> Vec<Event> {
        match op {
            Op::UserTurn { input } => {
                let turn_id = self.next_turn_id;
                self.next_turn_id += 1;
                let (input_preview, input_truncated) =
                    summarize_user_input(&input, USER_INPUT_LOG_PREVIEW_CHARS);
                info!(
                    turn_id = turn_id,
                    phase = "received",
                    input_len = input.chars().count(),
                    input_preview = ?input_preview,
                    input_truncated = input_truncated,
                    "user_turn received"
                );
                self.record_history("user", input.clone());
                // Duplicate-call protection should only apply within one user turn.
                // Keeping records across turns can incorrectly block legitimate reads/writes.
                self.recent_tool_calls.clear();
                self.state_epoch = 0;

                let mut events = Vec::new();
                self.push_event(&mut events, Event::TurnStarted { turn_id });

                if let Some(call) = tools::parse_tool_call(&input) {
                    self.execute_single_tool_call(turn_id, call, &mut events)
                        .await;
                } else {
                    self.execute_natural_language_turn(turn_id, &input, &mut events)
                        .await;
                }

                self.push_event(&mut events, Event::TurnCompleted { turn_id });
                events
            }
            // Multi-agent operations (预留扩展)
            Op::SpawnAgent {
                input: _,
                source: _,
            } => {
                // Check depth limit
                if self.depth >= tools::MAX_AGENT_DEPTH {
                    return vec![Event::AssistantMessage {
                        turn_id: self.next_turn_id,
                        content: format!(
                            "Cannot spawn sub-agent: max depth {} reached",
                            tools::MAX_AGENT_DEPTH
                        ),
                    }];
                }

                let new_thread_id = ThreadId::new();
                self.next_turn_id += 1;

                vec![Event::AgentSpawned {
                    parent_thread_id: Some(self.thread_id),
                    new_thread_id,
                }]
            }
            Op::SendToAgent {
                thread_id: _,
                input: _,
            } => {
                // 预留扩展：向指定代理发送消息
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "SendToAgent not yet implemented".to_string(),
                }]
            }
            Op::InterruptAgent { thread_id: _ } => {
                // 预留扩展：中断指定代理
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "InterruptAgent not yet implemented".to_string(),
                }]
            }
            Op::ResumeAgent {
                rollout_path: _,
                source: _,
            } => {
                // 预留扩展：从持久化状态恢复代理
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "ResumeAgent not yet implemented".to_string(),
                }]
            }
            Op::Shutdown => vec![Event::ShutdownComplete],
        }
    }

    async fn execute_single_tool_call(
        &mut self,
        turn_id: u64,
        call: tools::ToolCall,
        events: &mut Vec<Event>,
    ) {
        let retry_config = RetryConfig::default();
        let start_time = Instant::now();

        info!(
            turn_id = turn_id,
            tool_name = %call.name,
            args = ?call.args,
            "tool_call started"
        );

        self.push_event(events, Event::ToolCallStarted {
            turn_id,
            tool_name: call.name.clone(),
        });

        // Try execution with retry
        let mut last_error = None;
        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                // Calculate delay with exponential backoff
                let delay = std::cmp::min(
                    retry_config.initial_delay_ms * 2u64.pow(attempt - 1),
                    retry_config.max_delay_ms,
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                self.push_event(events, Event::AssistantMessage {
                    turn_id,
                    content: format!("tool {} 第 {} 次重试...", call.name, attempt),
                });
                warn!(
                    turn_id = turn_id,
                    tool_name = %call.name,
                    attempt = attempt,
                    "tool_call retry"
                );
            }

            let (tool_event_tx, mut tool_event_rx) = tokio::sync::mpsc::unbounded_channel();
            match self
                .tools
                .execute(
                    turn_id,
                    &call,
                    self.cwd.as_path(),
                    self.tool_runtime_config,
                    self.approval_handler.clone(),
                    Some(tool_event_tx),
                )
                .await
            {
                Ok(output) => {
                    self.drain_tool_events(&mut tool_event_rx, events);
                    let duration_ms = start_time.elapsed().as_millis();
                    info!(
                        turn_id = turn_id,
                        tool_name = %call.name,
                        ok = true,
                        duration_ms = duration_ms,
                        output_len = output.len(),
                        "tool_call completed"
                    );
                    if attempt > 0 {
                        self.push_event(events, Event::AssistantMessage {
                            turn_id,
                            content: format!("tool {} 重试成功", call.name),
                        });
                    }
                    self.push_event(events, Event::ToolCallCompleted {
                        turn_id,
                        tool_name: call.name.clone(),
                        ok: true,
                        output,
                    });
                    let message = format!("tool {} 执行成功", call.name);
                    self.push_event(events, Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    });
                    self.record_history("assistant", message);
                    return;
                }
                Err(err) => {
                    self.drain_tool_events(&mut tool_event_rx, events);
                    last_error = Some(err);
                    // Check if error is retryable (not a validation error)
                    let err_str = last_error.as_ref().unwrap().to_string();
                    if err_str.contains("invalid")
                        || err_str.contains("permission denied")
                        || err_str.contains("Approval rejected")
                    {
                        // Non-retryable error, don't retry
                        break;
                    }
                }
            }
        }

        // All retries failed
        if let Some(err) = last_error {
            let duration_ms = start_time.elapsed().as_millis();
            info!(
                turn_id = turn_id,
                tool_name = %call.name,
                ok = false,
                duration_ms = duration_ms,
                error = %err,
                "tool_call completed"
            );
            self.push_event(events, Event::ToolCallCompleted {
                turn_id,
                tool_name: call.name.clone(),
                ok: false,
                output: err.to_string(),
            });
            let message = format!("tool {} 执行失败: {}", call.name, err);
            self.push_event(events, Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            });
            self.record_history("assistant", message);
        }
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
                    self.push_event(events, Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    });
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
                        self.push_event(events, Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        });
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
                    self.push_event(events, Event::AssistantMessage {
                        turn_id,
                        content: model_output.clone(),
                    });
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
                self.push_event(events, Event::AssistantMessage {
                    turn_id,
                    content: message.clone(),
                });
                self.record_history("assistant", message);
                return;
            }

            if action == "tool" {
                let tool_name = match decision.tool {
                    Some(name) if !name.trim().is_empty() => name,
                    _ => {
                        warn!(turn_id = turn_id, "model_decision missing tool name");
                        let message = "[model error] tool action missing tool name".to_string();
                        self.push_event(events, Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        });
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
                    self.push_event(events, Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    });
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
                        self.push_event(events, Event::AssistantMessage {
                            turn_id,
                            content: loop_message.clone(),
                        });
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

                self.push_event(events, Event::ToolCallStarted {
                    turn_id,
                    tool_name: tool_name.clone(),
                });

                let (tool_event_tx, mut tool_event_rx) = tokio::sync::mpsc::unbounded_channel();
                match self
                    .tools
                    .execute(
                        turn_id,
                        &call,
                        self.cwd.as_path(),
                        self.tool_runtime_config,
                        self.approval_handler.clone(),
                        Some(tool_event_tx),
                    )
                    .await
                {
                    Ok(output) => {
                        self.drain_tool_events(&mut tool_event_rx, events);
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

                        self.push_event(events, Event::ToolCallCompleted {
                            turn_id,
                            tool_name: tool_name.to_string(),
                            ok: true,
                            output: output.to_string(),
                        });
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
                    }
                    Err(err) => {
                        self.drain_tool_events(&mut tool_event_rx, events);
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

                        self.push_event(events, Event::ToolCallCompleted {
                            turn_id,
                            tool_name: tool_name.to_string(),
                            ok: false,
                            output: err_text.to_string(),
                        });
                        executed_count += 1;
                        consecutive_duplicate_skips = 0;
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
            self.push_event(events, Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            });
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
        self.push_event(events, Event::AssistantMessage {
            turn_id,
            content: message.clone(),
        });
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
                self.push_event(events, Event::AssistantDelta {
                    turn_id,
                    content_delta: delta,
                });
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

    fn drain_tool_events(
        &self,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
        events: &mut Vec<Event>,
    ) {
        while let Ok(event) = rx.try_recv() {
            self.push_event(events, event);
        }
    }
}

fn truncate_for_prompt(text: &str) -> String {
    if text.chars().count() <= MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT {
        return text.to_string();
    }

    let snippet = text
        .chars()
        .take(MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT)
        .collect::<String>();
    format!("{snippet}...")
}

fn summarize_user_input(input: &str, preview_limit: usize) -> (String, bool) {
    let normalized = input.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= preview_limit {
        return (normalized, false);
    }

    let mut preview = normalized.chars().take(preview_limit).collect::<String>();
    preview.push_str("...");
    (preview, true)
}

fn extract_json_candidate(raw: &str) -> String {
    let trimmed = raw.trim();

    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines().collect::<Vec<_>>();
    if lines.first().is_some_and(|line| line.starts_with("```")) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| line.trim() == "```") {
        lines.pop();
    }

    lines.join("\n")
}

fn extract_json_object_from_mixed_text(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

fn parse_model_decision(raw: &str) -> Option<ModelDecision> {
    // Case 1: pure JSON or fenced JSON.
    let candidate = extract_json_candidate(raw);
    if let Ok(parsed) = serde_json::from_str::<ModelDecision>(&candidate) {
        return Some(parsed);
    }

    // Case 2: mixed text (e.g. reasoning + trailing JSON object).
    let mixed = extract_json_object_from_mixed_text(raw)?;
    serde_json::from_str::<ModelDecision>(&mixed).ok()
}

fn is_supported_tool_name(name: &str) -> bool {
    matches!(
        name,
        "read_file" | "list_dir" | "grep_files" | "shell" | "apply_patch" | "edit_file_range"
    )
}

fn stringify_json_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn normalize_model_decision(mut decision: ModelDecision) -> ModelDecision {
    let action_lower = decision.action.to_ascii_lowercase();
    if action_lower == "tool" || action_lower == "final" {
        return decision;
    }

    if !is_supported_tool_name(&action_lower) {
        return decision;
    }

    if decision
        .tool
        .as_deref()
        .map_or(true, |t| t.trim().is_empty())
    {
        decision.tool = Some(action_lower.clone());
    }

    if decision.args.is_none() {
        let mut args = HashMap::new();
        for (k, v) in &decision.extra {
            if matches!(k.as_str(), "action" | "type" | "tool" | "args" | "message") {
                continue;
            }
            if let Some(value) = stringify_json_value(v) {
                args.insert(k.clone(), value);
            }
        }
        if !args.is_empty() {
            decision.args = Some(args);
        }
    }

    decision.action = "tool".to_string();
    decision
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

fn build_planner_input(
    user_input: &str,
    history: &[HistoryEntry],
    tool_traces: &[String],
    remaining_calls: usize,
) -> String {
    let history_context = if history.is_empty() {
        "(no prior turns)".to_string()
    } else {
        history
            .iter()
            .map(|item| format!("{}: {}", item.role, item.content))
            .collect::<Vec<String>>()
            .join("\n")
    };

    let tool_context = if tool_traces.is_empty() {
        "(no tools executed yet)".to_string()
    } else {
        tool_traces.join("\n")
    };

    format!(
        "You are OpenJax's planning layer.\n\
Return ONLY valid JSON with one of two shapes:\n\
1) Tool call: {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|shell|apply_patch|edit_file_range\",\"args\":{{...}}}}\n\
2) Final answer: {{\"action\":\"final\",\"message\":\"...\"}}\n\
\n\
Rules:\n\
- At most one action per response.\n\
- You can call tools up to {remaining_calls} more times this turn.\n\
- If task can be answered now, return final.\n\
- For shell, put shell command in args.cmd.\n\
- For apply_patch, use this EXACT format (note the space prefix for context lines):\n\
  *** Begin Patch\n\
  *** Update File: <filepath>\n\
  @@\n\
   context line (MUST start with space)\n\
  -line to remove (starts with -)\n\
  +line to add (starts with +)\n\
  *** End Patch\n\
  Operations: *** Add File:, *** Update File:, *** Delete File:, *** Move File: from -> to\n\
  IMPORTANT: In Update File, every line after @@ MUST start with space (context), - (remove), or + (add).\n\
- For edit_file_range, provide args: file_path, start_line, end_line, new_text.\n\
- IMPORTANT: Do NOT repeat the same tool call with the same arguments. Check the tool execution history carefully.\n\
- If a tool was already called and returned results, use those results to decide the next action.\n\
- Only call a tool again if you need different arguments or if the previous call failed.\n\
- After a successful apply_patch, at most one read_file call is allowed for verification, then return final.\n\
- If verification already shows the requested content/changes are present, return final immediately.\n\
\n\
Conversation history (most recent last):\n{history_context}\n\
\n\
User request:\n{user_input}\n\
\n\
Tool execution history:\n{tool_context}\n"
    )
}

fn build_final_response_prompt(user_input: &str, tool_traces: &[String], seed_message: &str) -> String {
    let tool_context = if tool_traces.is_empty() {
        "(no tools executed in this turn)".to_string()
    } else {
        tool_traces.join("\n")
    };

    format!(
        "You are OpenJax's final response writer.\n\
Produce only the final assistant reply text for the user.\n\
Do not output JSON, markdown fences, or extra metadata.\n\
Keep the response concise, accurate, and actionable.\n\
\n\
User request:\n{user_input}\n\
\n\
Tool execution summary for this turn:\n{tool_context}\n\
\n\
Draft answer from planner:\n{seed_message}\n"
    )
}

fn build_json_repair_prompt(previous_output: &str) -> String {
    format!(
        "Your previous response did not match the required JSON schema.\n\
Return ONLY valid JSON. Do not include markdown, thoughts, or extra text.\n\
Allowed outputs:\n\
1) {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|shell|apply_patch|edit_file_range\",\"args\":{{...}}}}\n\
2) {{\"action\":\"final\",\"message\":\"...\"}}\n\
\n\
Previous response:\n{previous_output}\n"
    )
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_approval_policy(value: &str) -> Option<tools::ApprovalPolicy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "always_ask" => Some(tools::ApprovalPolicy::AlwaysAsk),
        "on_request" => Some(tools::ApprovalPolicy::OnRequest),
        "never" => Some(tools::ApprovalPolicy::Never),
        _ => None,
    }
}

fn parse_sandbox_mode(value: &str) -> Option<tools::SandboxMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "workspace_write" => Some(tools::SandboxMode::WorkspaceWrite),
        "danger_full_access" => Some(tools::SandboxMode::DangerFullAccess),
        _ => None,
    }
}

fn resolve_approval_policy(config: &Config) -> tools::ApprovalPolicy {
    if let Ok(val) = std::env::var("OPENJAX_APPROVAL_POLICY") {
        if let Some(policy) = parse_approval_policy(&val) {
            return policy;
        }
    }

    if let Some(policy) = config
        .sandbox
        .as_ref()
        .and_then(|s| s.approval_policy.as_deref())
        .and_then(parse_approval_policy)
    {
        return policy;
    }

    tools::ApprovalPolicy::OnRequest
}

fn resolve_sandbox_mode(config: &Config) -> tools::SandboxMode {
    if let Ok(val) = std::env::var("OPENJAX_SANDBOX_MODE") {
        if let Some(mode) = parse_sandbox_mode(&val) {
            return mode;
        }
    }

    if let Some(mode) = config
        .sandbox
        .as_ref()
        .and_then(|s| s.mode.as_deref())
        .and_then(parse_sandbox_mode)
    {
        return mode;
    }

    tools::SandboxMode::WorkspaceWrite
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
        Agent, ApprovalPolicy, SandboxMode, build_planner_input, model::ModelClient,
        normalize_model_decision,
        parse_approval_policy, parse_model_decision, parse_sandbox_mode,
        should_abort_on_consecutive_duplicate_skips, summarize_user_input,
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
        assert!(first_delta_index.is_some(), "expected assistant delta events");
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
