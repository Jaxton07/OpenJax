use serde::{Deserialize, Serialize};

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
pub enum Event {
    TurnStarted {
        turn_id: u64,
    },
    ToolCallStarted {
        turn_id: u64,
        tool_name: String,
    },
    ToolCallCompleted {
        turn_id: u64,
        tool_name: String,
        ok: bool,
        output: String,
    },
    AssistantMessage {
        turn_id: u64,
        content: String,
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
    TurnCompleted {
        turn_id: u64,
    },
    ShutdownComplete,
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
