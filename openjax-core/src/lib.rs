mod config;
mod model;
mod tools;

pub use config::Config;
use openjax_protocol::{Event, Op};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

pub use model::build_model_client;
pub use tools::ApprovalPolicy;
pub use tools::SandboxMode;
pub use tools::{AgentConfig, AgentRuntime, MAX_AGENT_DEPTH};

// Re-export protocol types for external use
pub use openjax_protocol::{AgentSource, AgentStatus, ThreadId};

const MAX_TOOL_CALLS_PER_TURN: usize = 5;
const MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT: usize = 4_000;
const MAX_CONVERSATION_HISTORY_ITEMS: usize = 20;

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
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    role: &'static str,
    content: String,
}

pub struct Agent {
    next_turn_id: u64,
    model_client: Box<dyn model::ModelClient>,
    tools: tools::ToolRouter,
    tool_runtime_config: tools::ToolRuntimeConfig,
    cwd: PathBuf,
    history: Vec<HistoryEntry>,
    // Multi-agent support (预留扩展)
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    depth: i32,
}

impl Agent {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::with_runtime(
            tools::ApprovalPolicy::from_env(),
            tools::SandboxMode::from_env(),
            cwd,
        )
    }

    pub fn with_runtime(
        approval_policy: tools::ApprovalPolicy,
        sandbox_mode: tools::SandboxMode,
        cwd: PathBuf,
    ) -> Self {
        let model_client = model::build_model_client();
        Self {
            next_turn_id: 1,
            model_client,
            tools: tools::ToolRouter::new(),
            tool_runtime_config: tools::ToolRuntimeConfig {
                approval_policy,
                sandbox_mode,
            },
            cwd,
            history: Vec::new(),
            // Multi-agent support (预留扩展)
            thread_id: ThreadId::new(),
            parent_thread_id: None,
            depth: 0,
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
    pub fn spawn_sub_agent(&self, input: &str) -> Result<Agent, String> {
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

        Ok(sub_agent)
    }

    // ============== Main Submit Method ==============

    pub async fn submit(&mut self, op: Op) -> Vec<Event> {
        match op {
            Op::UserTurn { input } => {
                let turn_id = self.next_turn_id;
                self.next_turn_id += 1;
                self.record_history("user", input.clone());

                let mut events = vec![Event::TurnStarted { turn_id }];

                if let Some(call) = tools::parse_tool_call(&input) {
                    self.execute_single_tool_call(turn_id, call, &mut events)
                        .await;
                } else {
                    self.execute_natural_language_turn(turn_id, &input, &mut events)
                        .await;
                }

                events.push(Event::TurnCompleted { turn_id });
                events
            }
            // Multi-agent operations (预留扩展)
            Op::SpawnAgent { input, source } => {
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
                let turn_id = self.next_turn_id;
                self.next_turn_id += 1;

                vec![Event::AgentSpawned {
                    parent_thread_id: Some(self.thread_id),
                    new_thread_id,
                }]
            }
            Op::SendToAgent { thread_id: _, input: _ } => {
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
            Op::ResumeAgent { rollout_path: _, source: _ } => {
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

        events.push(Event::ToolCallStarted {
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
                events.push(Event::AssistantMessage {
                    turn_id,
                    content: format!("tool {} 第 {} 次重试...", call.name, attempt),
                });
            }

            match self
                .tools
                .execute(&call, self.cwd.as_path(), self.tool_runtime_config)
                .await
            {
                Ok(output) => {
                    if attempt > 0 {
                        events.push(Event::AssistantMessage {
                            turn_id,
                            content: format!("tool {} 重试成功", call.name),
                        });
                    }
                    events.push(Event::ToolCallCompleted {
                        turn_id,
                        tool_name: call.name.clone(),
                        ok: true,
                        output,
                    });
                    let message = format!("tool {} 执行成功", call.name);
                    events.push(Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    });
                    self.record_history("assistant", message);
                    return;
                }
                Err(err) => {
                    last_error = Some(err);
                    // Check if error is retryable (not a validation error)
                    let err_str = last_error.as_ref().unwrap().to_string();
                    if err_str.contains("invalid") || err_str.contains("permission denied") {
                        // Non-retryable error, don't retry
                        break;
                    }
                }
            }
        }

        // All retries failed
        if let Some(err) = last_error {
            events.push(Event::ToolCallCompleted {
                turn_id,
                tool_name: call.name.clone(),
                ok: false,
                output: err.to_string(),
            });
            let message = format!("tool {} 执行失败: {}", call.name, err);
            events.push(Event::AssistantMessage {
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

        for executed_count in 0..MAX_TOOL_CALLS_PER_TURN {
            let remaining = MAX_TOOL_CALLS_PER_TURN - executed_count;
            let planner_input =
                build_planner_input(user_input, &self.history, &tool_traces, remaining);

            let model_output = match self.model_client.complete(&planner_input).await {
                Ok(output) => output,
                Err(err) => {
                    let message = format!("[model error] {err}");
                    events.push(Event::AssistantMessage {
                        turn_id,
                        content: message.clone(),
                    });
                    self.record_history("assistant", message);
                    return;
                }
            };

            let decision = if let Some(parsed) = parse_model_decision(&model_output) {
                parsed
            } else {
                let repair_prompt = build_json_repair_prompt(&model_output);
                let repaired_output = match self.model_client.complete(&repair_prompt).await {
                    Ok(output) => output,
                    Err(err) => {
                        let message = format!("[model error] {err}");
                        events.push(Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        });
                        self.record_history("assistant", message);
                        return;
                    }
                };

                if let Some(parsed) = parse_model_decision(&repaired_output) {
                    parsed
                } else {
                    // Still non-JSON after one repair attempt: treat first output as final.
                    events.push(Event::AssistantMessage {
                        turn_id,
                        content: model_output.clone(),
                    });
                    self.record_history("assistant", model_output);
                    return;
                }
            };

            let action = decision.action.to_ascii_lowercase();
            if action == "final" {
                let message = decision
                    .message
                    .unwrap_or_else(|| "任务已完成。".to_string());
                events.push(Event::AssistantMessage {
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
                        let message = "[model error] tool action missing tool name".to_string();
                        events.push(Event::AssistantMessage {
                            turn_id,
                            content: message.clone(),
                        });
                        self.record_history("assistant", message);
                        return;
                    }
                };

                let call = tools::ToolCall {
                    name: tool_name.clone(),
                    args: decision.args.unwrap_or_default(),
                };

                events.push(Event::ToolCallStarted {
                    turn_id,
                    tool_name: tool_name.clone(),
                });

                match self
                    .tools
                    .execute(&call, self.cwd.as_path(), self.tool_runtime_config)
                    .await
                {
                    Ok(output) => {
                        let trace = format!(
                            "tool={tool_name}; ok=true; output={}",
                            truncate_for_prompt(&output)
                        );
                        tool_traces.push(trace);

                        events.push(Event::ToolCallCompleted {
                            turn_id,
                            tool_name,
                            ok: true,
                            output,
                        });
                    }
                    Err(err) => {
                        let err_text = err.to_string();
                        let trace = format!(
                            "tool={tool_name}; ok=false; output={}",
                            truncate_for_prompt(&err_text)
                        );
                        tool_traces.push(trace);

                        events.push(Event::ToolCallCompleted {
                            turn_id,
                            tool_name,
                            ok: false,
                            output: err_text,
                        });
                    }
                }

                continue;
            }

            let message = format!("[model error] unsupported action: {}", decision.action);
            events.push(Event::AssistantMessage {
                turn_id,
                content: message.clone(),
            });
            self.record_history("assistant", message);
            return;
        }

        let message = format!(
            "已达到单回合最多 {} 次工具调用限制，请继续下一轮。",
            MAX_TOOL_CALLS_PER_TURN
        );
        events.push(Event::AssistantMessage {
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
1) Tool call: {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|exec_command|apply_patch\",\"args\":{{...}}}}\n\
2) Final answer: {{\"action\":\"final\",\"message\":\"...\"}}\n\
\
Rules:\n\
- At most one action per response.\n\
- You can call tools up to {remaining_calls} more times this turn.\n\
- If task can be answered now, return final.\n\
- For exec_command, put shell command in args.cmd.\n\
- For apply_patch, put full patch text in args.patch.\n\
\
Conversation history (most recent last):\n{history_context}\n\
\
User request:\n{user_input}\n\
\
Tool execution history:\n{tool_context}\n"
    )
}

fn build_json_repair_prompt(previous_output: &str) -> String {
    format!(
        "Your previous response did not match the required JSON schema.\n\
Return ONLY valid JSON. Do not include markdown, thoughts, or extra text.\n\
Allowed outputs:\n\
1) {{\"action\":\"tool\",\"tool\":\"read_file|list_dir|grep_files|exec_command|apply_patch\",\"args\":{{...}}}}\n\
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
