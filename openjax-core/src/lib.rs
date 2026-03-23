mod agent;
pub mod approval;
pub mod builtin_catalog;
mod config;
pub mod dispatcher;
mod logger;
mod model;
mod paths;
mod provider_store;
pub mod sandbox;
pub mod skills;
pub mod slash_commands;
pub mod streaming;
pub mod tools;

use agent::state::{RateLimitConfig, ToolCallRecord};
pub use approval::{ApprovalHandler, ApprovalRequest, StdinApprovalHandler};
pub use approval::{DEFAULT_APPROVAL_TIMEOUT_MS, approval_timeout_ms_from_env};
pub use builtin_catalog::{BUILTIN_CATALOG, CatalogModel, CatalogProvider};
pub use config::AgentConfig;
pub use config::Config;
pub use config::ModelConfig;
pub use config::ModelRoutingConfig;
pub use config::ProviderModelConfig;
pub use config::SkillsConfig;
pub use logger::init_logger;
pub use logger::init_logger_with_file;
pub use logger::init_split_logger;
use openjax_policy::runtime::PolicyRuntime;
use openjax_protocol::Event;
pub use paths::OpenJaxPaths;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub use model::build_model_client;
pub use model::build_model_client_with_config;
pub use provider_store::{
    build_config_from_providers, load_runtime_config, normalize_model_id, provider_protocol,
    provider_vendor,
};
pub use tools::ApprovalPolicy;
pub use tools::SandboxMode;

// Re-export loop detector types for integration tests
pub use agent::loop_detector::{LoopDetector, LoopSignal};

// Re-export protocol types for external use
pub use openjax_protocol::{AgentSource, AgentStatus, ThreadId};

const MAX_CONSECUTIVE_DUPLICATE_SKIPS: usize = 2;
pub(crate) const MAX_TOOL_OUTPUT_CHARS_FOR_PROMPT: usize = 4_000;
pub(crate) const MAX_CONVERSATION_HISTORY_TURNS: usize = 100;
pub(crate) const SYNTHETIC_ASSISTANT_DELTA_CHUNK_CHARS: usize = 24;
const USER_INPUT_LOG_PREVIEW_CHARS: usize = 200;

#[derive(Debug, Clone)]
pub(crate) struct TurnRecord {
    pub(crate) user_input: String,
    pub(crate) tool_traces: Vec<String>,
    pub(crate) assistant_output: String,
}

#[derive(Debug, Clone)]
pub(crate) enum HistoryItem {
    Turn(TurnRecord),
    #[allow(dead_code)]
    Summary(String),
}

pub struct Agent {
    next_turn_id: u64,
    model_client: Box<dyn model::ModelClient>,
    tools: tools::ToolRouter,
    tool_runtime_config: tools::ToolRuntimeConfig,
    skill_registry: skills::SkillRegistry,
    skill_runtime_config: skills::SkillRuntimeConfig,
    cwd: PathBuf,
    history: Vec<HistoryItem>,
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    depth: i32,
    last_api_call_time: Option<std::time::Instant>,
    rate_limit_config: RateLimitConfig,
    max_tool_calls_per_turn: usize,
    loop_detector: crate::agent::loop_detector::LoopDetector,
    max_planner_rounds_per_turn: usize,
    recent_tool_calls: Vec<ToolCallRecord>,
    state_epoch: u64,
    dispatcher_config: dispatcher::DispatcherConfig,
    tool_batch_v2_enabled: bool,
    approval_handler: Arc<dyn approval::ApprovalHandler>,
    event_sink: Option<UnboundedSender<Event>>,
    policy_runtime: Option<PolicyRuntime>,
    policy_session_id: Option<String>,
    context_window_size: u32,
    last_input_tokens: Option<u64>,
}

impl Agent {}

#[cfg(test)]
mod tests;
