mod agent;
pub mod approval;
mod config;
mod logger;
mod model;
pub mod tools;

use agent::state::{RateLimitConfig, ToolCallRecord};
pub use approval::{ApprovalHandler, ApprovalRequest, StdinApprovalHandler};
pub use config::AgentConfig;
pub use config::Config;
pub use logger::init_logger;
use openjax_protocol::Event;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

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
pub(crate) const MAX_CONVERSATION_HISTORY_ITEMS: usize = 20;
pub(crate) const SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS: usize = 24;
const USER_INPUT_LOG_PREVIEW_CHARS: usize = 200;

#[derive(Debug, Clone)]
pub(crate) struct HistoryEntry {
    pub(crate) role: &'static str,
    pub(crate) content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalResponseMode {
    FinalWriter,
    PlannerOnly,
}

impl FinalResponseMode {
    fn from_env() -> Self {
        let raw = std::env::var("OPENJAX_FINAL_WRITER")
            .ok()
            .unwrap_or_else(|| "off".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" | "planner" | "planner_only" => Self::PlannerOnly,
            _ => Self::FinalWriter,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FinalWriter => "final_writer",
            Self::PlannerOnly => "planner_only",
        }
    }
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
    final_response_mode: FinalResponseMode,
    approval_handler: Arc<dyn approval::ApprovalHandler>,
    event_sink: Option<UnboundedSender<Event>>,
}

impl Agent {}

#[cfg(test)]
mod tests;
