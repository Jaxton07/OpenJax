use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

use crate::agent::runtime_policy::{resolve_approval_policy, resolve_sandbox_mode};
use crate::agent::state::RateLimitConfig;
use crate::{Agent, Config, approval, model, tools};

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
        let thread_id = crate::ThreadId::new();
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
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}
