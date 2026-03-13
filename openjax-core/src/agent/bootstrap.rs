use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

use crate::agent::runtime_policy::{
    resolve_approval_policy, resolve_max_planner_rounds_per_turn, resolve_max_tool_calls_per_turn,
    resolve_sandbox_mode,
};
use crate::agent::state::RateLimitConfig;
use crate::{Agent, Config, FinalResponseMode, approval, model, skills, tools};

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
            approval_policy = approval_policy.as_str(),
            sandbox_mode = sandbox_mode.as_str(),
            max_tool_calls_per_turn = max_tool_calls_per_turn,
            max_planner_rounds_per_turn = max_planner_rounds_per_turn,
            skills_enabled = skill_runtime_config.enabled,
            skills_loaded = skill_registry.len(),
            prevent_shell_skill_trigger = skill_runtime_config.prevent_shell_skill_trigger,
            prefer_lightweight_git_inspection =
                skill_runtime_config.prefer_lightweight_git_inspection,
            max_diff_chars_for_planner = skill_runtime_config.max_diff_chars_for_planner,
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
                prevent_shell_skill_trigger: skill_runtime_config.prevent_shell_skill_trigger,
            }),
            tool_runtime_config: tools::ToolRuntimeConfig {
                approval_policy,
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
            max_planner_rounds_per_turn,
            recent_tool_calls: Vec::new(),
            state_epoch: 0,
            final_response_mode: FinalResponseMode::from_env(),
            stream_engine_v2_enabled: true,
            tool_batch_v2_enabled: true,
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
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}
