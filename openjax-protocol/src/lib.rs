use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ============== Agent Types ==============

/// Unique identifier for a thread/agent
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ThreadId(u64);

impl ThreadId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

/// Source of an agent (used for multi-agent scenarios)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentSource {
    /// Root agent started from CLI
    Root,
    /// Sub-agent spawned from another agent
    SubAgent {
        parent_thread_id: ThreadId,
        depth: i32,
    },
}

/// Maximum depth for sub-agent spawning (参考 Codex: MAX_THREAD_SPAWN_DEPTH)
pub const MAX_AGENT_DEPTH: i32 = 1;

// ============== Operation Types ==============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Op {
    UserTurn {
        input: String,
    },
    /// Spawn a new sub-agent (预留扩展)
    SpawnAgent {
        input: String,
        source: AgentSource,
    },
    /// Send input to existing agent (预留扩展)
    SendToAgent {
        thread_id: ThreadId,
        input: String,
    },
    /// Interrupt a running agent (预留扩展)
    InterruptAgent {
        thread_id: ThreadId,
    },
    /// Resume agent from persisted state (预留扩展)
    ResumeAgent {
        rollout_path: String,
        source: AgentSource,
    },
    Shutdown,
}

// ============== Event Types ==============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellExecutionMetadata {
    pub result_class: String,
    pub backend: String,
    pub exit_code: i32,
    pub policy_decision: String,
    pub runtime_allowed: bool,
    pub degrade_reason: Option<String>,
    pub runtime_deny_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Event {
    TurnStarted {
        turn_id: u64,
    },
    ToolCallStarted {
        turn_id: u64,
        /// Stable identifier for one full tool invocation lifecycle.
        /// The matching ToolCallCompleted event MUST carry the same id.
        tool_call_id: String,
        tool_name: String,
        target: Option<String>,
        #[serde(default)]
        display_name: Option<String>,
    },
    ToolCallCompleted {
        turn_id: u64,
        /// Stable identifier for one full tool invocation lifecycle.
        /// Must be identical to the corresponding ToolCallStarted id.
        tool_call_id: String,
        tool_name: String,
        ok: bool,
        output: String,
        #[serde(default)]
        shell_metadata: Option<ShellExecutionMetadata>,
        #[serde(default)]
        display_name: Option<String>,
    },
    ToolCallArgsDelta {
        turn_id: u64,
        tool_call_id: String,
        tool_name: String,
        args_delta: String,
        #[serde(default)]
        display_name: Option<String>,
    },
    ToolCallReady {
        turn_id: u64,
        tool_call_id: String,
        tool_name: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        target: Option<String>,
    },
    ToolCallProgress {
        turn_id: u64,
        tool_call_id: String,
        tool_name: String,
        progress_message: String,
        #[serde(default)]
        display_name: Option<String>,
    },
    ToolCallFailed {
        turn_id: u64,
        tool_call_id: String,
        tool_name: String,
        code: String,
        message: String,
        retryable: bool,
        #[serde(default)]
        display_name: Option<String>,
    },
    /// Deprecated compatibility-only assistant message event.
    ///
    /// A: legacy bridge stage, where producers may still emit this alongside
    /// `ResponseCompleted` for older consumers.
    /// B: legacy fallback stage, where new logic should treat it as optional.
    /// C: removal target for the compatibility surface.
    AssistantMessage {
        turn_id: u64,
        content: String,
    },
    ResponseStarted {
        turn_id: u64,
        stream_source: StreamSource,
    },
    ResponseTextDelta {
        turn_id: u64,
        content_delta: String,
        stream_source: StreamSource,
    },
    ReasoningDelta {
        turn_id: u64,
        content_delta: String,
        stream_source: StreamSource,
    },
    ToolCallsProposed {
        turn_id: u64,
        tool_calls: Vec<ToolCallProposal>,
    },
    ToolBatchCompleted {
        turn_id: u64,
        total: u32,
        succeeded: u32,
        failed: u32,
    },
    ResponseResumed {
        turn_id: u64,
        stream_source: StreamSource,
    },
    ResponseCompleted {
        turn_id: u64,
        content: String,
        stream_source: StreamSource,
    },
    LoopWarning {
        turn_id: u64,
        tool_name: String,
        consecutive_count: usize,
    },
    ResponseError {
        turn_id: u64,
        code: String,
        message: String,
        retryable: bool,
    },
    ApprovalRequested {
        turn_id: u64,
        request_id: String,
        target: String,
        reason: String,
        #[serde(default)]
        policy_version: Option<u64>,
        #[serde(default)]
        matched_rule_id: Option<String>,
        #[serde(default)]
        tool_name: Option<String>,
        #[serde(default)]
        command_preview: Option<String>,
        #[serde(default)]
        risk_tags: Vec<String>,
        #[serde(default)]
        sandbox_backend: Option<String>,
        #[serde(default)]
        degrade_reason: Option<String>,
        #[serde(default)]
        approval_kind: Option<ApprovalKind>,
    },
    ApprovalResolved {
        turn_id: u64,
        request_id: String,
        approved: bool,
    },
    ContextUsageUpdated {
        turn_id: u64,
        input_tokens: u64,
        context_window_size: u32,
        ratio: f64,
    },
    /// New agent spawned (预留扩展)
    AgentSpawned {
        parent_thread_id: Option<ThreadId>,
        new_thread_id: ThreadId,
    },
    /// Agent status changed (预留扩展)
    AgentStatusChanged {
        thread_id: ThreadId,
        status: AgentStatus,
    },
    ContextCompacted {
        turn_id: u64,
        compressed_turns: u32,   // 被压缩替换的 Turn 条数
        retained_turns: u32,     // 保留的最近 Turn 条数
        summary_preview: String, // 摘要前 120 字，供前端展示
    },
    TurnCompleted {
        turn_id: u64,
    },
    ShutdownComplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Normal,
    Escalation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamSource {
    ModelLive,
    Synthetic,
    Replay,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCallProposal {
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub arguments: BTreeMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub concurrency_group: Option<String>,
}

/// Agent status (预留扩展)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    PendingInit,
    Running,
    Completed(Option<String>),
    Errored(String),
    Interrupted,
    Shutdown,
    NotFound,
}
