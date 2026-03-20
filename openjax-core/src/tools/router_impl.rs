use anyhow::{Result, anyhow};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::context::SandboxPolicy;
use super::orchestrator::ToolOrchestrator;
use super::registry::ToolHandler;
use super::router::{ToolCall, ToolRuntimeConfig};
use super::spec::ToolSpec;
use super::tool_builder::{
    CreateToolInvocationParams, build_tool_registry_with_config, create_tool_invocation,
};
use crate::approval::ApprovalHandler;

#[derive(Debug, Clone)]
pub struct ToolExecOutcome {
    pub output: String,
    pub success: bool,
}

#[derive(Clone)]
pub struct ToolRouter {
    orchestrator: Arc<ToolOrchestrator>,
    specs: Vec<ToolSpec>,
}

pub struct ToolExecutionRequest<'a> {
    pub turn_id: u64,
    pub tool_call_id: String,
    pub call: &'a ToolCall,
    pub cwd: &'a std::path::Path,
    pub config: ToolRuntimeConfig,
    pub approval_handler: Arc<dyn ApprovalHandler>,
    pub event_sink: Option<tokio::sync::mpsc::UnboundedSender<openjax_protocol::Event>>,
}

impl ToolRouter {
    pub fn new() -> Self {
        let (registry, specs) =
            build_tool_registry_with_config(&crate::tools::spec::ToolsConfig::default());
        Self {
            orchestrator: Arc::new(ToolOrchestrator::new(Arc::new(registry))),
            specs,
        }
    }

    pub fn with_config(config: &crate::tools::spec::ToolsConfig) -> Self {
        let (registry, specs) = build_tool_registry_with_config(config);
        Self {
            orchestrator: Arc::new(ToolOrchestrator::new(Arc::new(registry))),
            specs,
        }
    }

    pub fn with_runtime_config(config: ToolRuntimeConfig) -> Self {
        let (registry, specs) = build_tool_registry_with_config(&config.tools_config);
        Self {
            orchestrator: Arc::new(ToolOrchestrator::new(Arc::new(registry))),
            specs,
        }
    }

    pub fn register_tool(&self, name: String, handler: Arc<dyn ToolHandler>) {
        self.orchestrator.register_tool(name, handler);
    }

    pub fn display_name_for(&self, tool_name: &str) -> Option<String> {
        self.specs.iter().find(|s| s.name == tool_name).map(|s| s.display_name.clone())
    }

    pub async fn execute(&self, request: ToolExecutionRequest<'_>) -> Result<ToolExecOutcome> {
        let ToolExecutionRequest {
            turn_id,
            tool_call_id,
            call,
            cwd,
            config,
            approval_handler,
            event_sink,
        } = request;
        debug!(
            tool_name = %call.name,
            args = ?call.args,
            args_json = %serde_json::to_string(&call.args).unwrap_or_default(),
            cwd = %cwd.display(),
            sandbox_mode = config.sandbox_mode.as_str(),
            approval_policy = config.approval_policy.as_str(),
            "tool_execute started"
        );

        let sandbox_policy = match config.sandbox_mode {
            super::router::SandboxMode::WorkspaceWrite => SandboxPolicy::Write,
            super::router::SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        };

        let approval_policy = config.approval_policy;

        let invocation = create_tool_invocation(CreateToolInvocationParams {
            turn_id,
            call_id: tool_call_id,
            tool_name: call.name.clone(),
            arguments: serde_json::to_string(&call.args)
                .map_err(|e| anyhow!("failed to serialize args: {}", e))?,
            cwd: cwd.to_path_buf(),
            sandbox_policy,
            approval_policy,
            shell_type: config.shell_type,
            prevent_shell_skill_trigger: config.prevent_shell_skill_trigger,
            approval_handler,
            event_sink,
        });

        let result = self.orchestrator.run(invocation).await;

        match result {
            Ok(tool_output) => match tool_output {
                super::context::ToolOutput::Function { body, success } => match body {
                    super::context::FunctionCallOutputBody::Text(text) => {
                        let success = success.unwrap_or(true);
                        let preview = summarize_preview(&text, 240);
                        info!(
                            tool_name = %call.name,
                            success = success,
                            output_len = text.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            output: text.clone(),
                            success,
                        })
                    }
                    super::context::FunctionCallOutputBody::Json(json) => {
                        let success = success.unwrap_or(true);
                        let text = json.to_string();
                        let preview = summarize_preview(&text, 240);
                        info!(
                            tool_name = %call.name,
                            success = success,
                            output_len = text.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            output: text,
                            success,
                        })
                    }
                },
                super::context::ToolOutput::Mcp { result, .. } => match result {
                    Ok(r) => {
                        let text = serde_json::to_string(&r)
                            .map_err(|e| anyhow!("failed to serialize mcp result: {}", e))?;
                        let preview = summarize_preview(&text, 240);
                        info!(
                            tool_name = %call.name,
                            success = true,
                            output_len = text.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            output: text,
                            success: true,
                        })
                    }
                    Err(e) => {
                        warn!(tool_name = %call.name, error = %e, "tool_execute failed");
                        Err(anyhow!("error: {}", e))
                    }
                },
            },
            Err(err) => {
                warn!(tool_name = %call.name, error = %err, "tool_execute failed");
                Err(anyhow!("error: {}", err))
            }
        }
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

fn summarize_preview(text: &str, limit: usize) -> String {
    let normalized = text.replace('\n', "\\n").replace('\r', "\\r");
    let total = normalized.chars().count();
    if total <= limit {
        return normalized;
    }
    let mut preview = normalized.chars().take(limit).collect::<String>();
    preview.push_str("...");
    preview
}
