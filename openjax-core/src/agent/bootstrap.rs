use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

use crate::agent::runtime_policy::{
    resolve_max_planner_rounds_per_turn, resolve_max_tool_calls_per_turn, resolve_sandbox_mode,
};
use crate::agent::state::RateLimitConfig;
use crate::dispatcher::DispatcherConfig;
use crate::{Agent, Config, approval, model, skills, tools};
use openjax_policy::runtime::PolicyRuntime;

impl Agent {
    pub fn new() -> Self {
        let config = Config::load();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::with_config_and_runtime(config, tools::SandboxMode::from_env(), cwd)
    }

    pub fn with_config(config: Config) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox_mode = resolve_sandbox_mode(&config);
        Self::with_config_and_runtime(config, sandbox_mode, cwd)
    }

    pub fn with_runtime(sandbox_mode: tools::SandboxMode, cwd: PathBuf) -> Self {
        let config = Config::load();
        Self::with_config_and_runtime(config, sandbox_mode, cwd)
    }

    pub fn with_config_and_runtime(
        config: Config,
        sandbox_mode: tools::SandboxMode,
        cwd: PathBuf,
    ) -> Self {
        let model_client = model::build_model_client_with_config(config.model.as_ref());
        let context_window_size: u32 = config
            .model
            .as_ref()
            .and_then(|mc| {
                mc.routing
                    .as_ref()
                    .and_then(|r| r.planner.as_ref())
                    .and_then(|id| mc.models.get(id.as_str()))
                    .and_then(|m| m.context_window_size)
            })
            .unwrap_or(0);
        let dispatcher_config = DispatcherConfig::from_env();
        let max_tool_calls_per_turn = resolve_max_tool_calls_per_turn(&config);
        let max_planner_rounds_per_turn = resolve_max_planner_rounds_per_turn(&config);
        let skill_config = config.skills.as_ref();
        let skill_runtime_config = skills::SkillRuntimeConfig::from_options(
            skill_config.and_then(|cfg| cfg.enabled),
            skill_config.and_then(|cfg| cfg.max_selected),
            skill_config.and_then(|cfg| cfg.max_prompt_chars),
            skill_config.and_then(|cfg| cfg.prevent_shell_skill_trigger),
            skill_config.and_then(|cfg| cfg.prefer_lightweight_git_inspection),
            skill_config.and_then(|cfg| cfg.max_diff_chars_for_planner),
        )
        .apply_env();
        let skill_registry = skills::SkillRegistry::load_from_default_locations();
        let thread_id = crate::ThreadId::new();
        info!(
            thread_id = ?thread_id,
            model_backend = model_client.name(),
            sandbox_mode = sandbox_mode.as_str(),
            max_tool_calls_per_turn = max_tool_calls_per_turn,
            max_planner_rounds_per_turn = max_planner_rounds_per_turn,
            skills_enabled = skill_runtime_config.enabled,
            skills_loaded = skill_registry.len(),
            prevent_shell_skill_trigger = skill_runtime_config.prevent_shell_skill_trigger,
            prefer_lightweight_git_inspection =
                skill_runtime_config.prefer_lightweight_git_inspection,
            max_diff_chars_for_planner = skill_runtime_config.max_diff_chars_for_planner,
            dispatcher_heuristic_detect = dispatcher_config.heuristic_detect,
            cwd = %cwd.display(),
            "agent created"
        );
        Self {
            next_turn_id: 1,
            model_client,
            tools: tools::ToolRouter::with_runtime_config(tools::ToolRuntimeConfig {
                sandbox_mode,
                shell_type: tools::ShellType::default(),
                tools_config: tools::spec::ToolsConfig::default(),
                prevent_shell_skill_trigger: skill_runtime_config.prevent_shell_skill_trigger,
            }),
            tool_runtime_config: tools::ToolRuntimeConfig {
                sandbox_mode,
                shell_type: tools::ShellType::default(),
                tools_config: tools::spec::ToolsConfig::default(),
                prevent_shell_skill_trigger: skill_runtime_config.prevent_shell_skill_trigger,
            },
            skill_registry,
            skill_runtime_config,
            cwd,
            history: Vec::new(),
            thread_id,
            parent_thread_id: None,
            depth: 0,
            last_api_call_time: None,
            rate_limit_config: RateLimitConfig::default(),
            max_tool_calls_per_turn,
            loop_detector: crate::agent::loop_detector::LoopDetector::new(),
            max_planner_rounds_per_turn,
            recent_tool_calls: Vec::new(),
            state_epoch: 0,
            dispatcher_config,
            tool_batch_v2_enabled: true,
            approval_handler: Arc::new(approval::StdinApprovalHandler::new()),
            event_sink: None,
            policy_runtime: None,
            policy_session_id: None,
            context_window_size,
            last_input_tokens: None,
        }
    }

    pub fn model_backend_name(&self) -> &'static str {
        self.model_client.name()
    }

    /// 返回当前 policy runtime 的默认决策名称，用于 TUI 展示。
    /// 无 policy runtime 时回退为 "ask"（保守默认，确保需要时触发审批）。
    pub fn policy_default_decision_name(&self) -> &'static str {
        self.policy_runtime
            .as_ref()
            .map(|r| r.handle().default_decision().as_str())
            .unwrap_or("ask")
    }

    pub fn sandbox_mode_name(&self) -> &'static str {
        self.tool_runtime_config.sandbox_mode.as_str()
    }

    pub fn set_approval_handler(&mut self, handler: Arc<dyn approval::ApprovalHandler>) {
        self.approval_handler = handler;
    }

    pub fn set_policy_runtime(&mut self, runtime: Option<PolicyRuntime>) {
        self.policy_runtime = runtime;
    }

    pub fn set_policy_session_id(&mut self, session_id: Option<String>) {
        self.policy_session_id = session_id;
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}
