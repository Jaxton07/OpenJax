use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};
use std::sync::Arc;

use super::router::{ToolCall, ToolRuntimeConfig};
use super::context::{SandboxPolicy};
use super::orchestrator::ToolOrchestrator;
use super::tool_builder::build_default_tool_registry;
use super::tool_builder::create_tool_invocation;
use super::registry::ToolHandler;

pub struct ToolRouter {
    orchestrator: Arc<ToolOrchestrator>,
}

impl ToolRouter {
    pub fn new() -> Self {
        let (registry, _) = build_default_tool_registry();
        Self {
            orchestrator: Arc::new(ToolOrchestrator::new(Arc::new(registry))),
        }
    }

    /// 注册动态工具
    pub fn register_tool(&self, name: String, handler: Arc<dyn ToolHandler>) {
        let orchestrator = Arc::as_ptr(&self.orchestrator) as *const ToolOrchestrator as *mut ToolOrchestrator;
        unsafe {
            (*orchestrator).register_tool(name, handler);
        }
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        cwd: &std::path::Path,
        config: ToolRuntimeConfig,
    ) -> Result<String> {
        debug!(
            tool_name = %call.name,
            args = ?call.args,
            cwd = %cwd.display(),
            sandbox_mode = config.sandbox_mode.as_str(),
            "tool_execute started"
        );

        let sandbox_policy = match config.sandbox_mode {
            super::router::SandboxMode::WorkspaceWrite => SandboxPolicy::Write,
            super::router::SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        };

        let invocation = create_tool_invocation(
            call.name.clone(),
            serde_json::to_string(&call.args).map_err(|e| anyhow!("failed to serialize args: {}", e))?,
            cwd.to_path_buf(),
            sandbox_policy,
        );

        let result = self.orchestrator.run(invocation).await;

        let output = match &result {
            Ok(tool_output) => {
                match tool_output {
                    super::context::ToolOutput::Function { body, .. } => {
                        match body {
                            super::context::FunctionCallOutputBody::Text(text) => {
                                info!(tool_name = %call.name, output_len = text.len(), "tool_execute completed");
                                text.clone()
                            }
                            super::context::FunctionCallOutputBody::Json(json) => {
                                info!(tool_name = %call.name, output_len = json.to_string().len(), "tool_execute completed");
                                json.to_string()
                            }
                        }
                    }
                    super::context::ToolOutput::Mcp { result, .. } => {
                        match result {
                            Ok(r) => {
                                info!(tool_name = %call.name, "tool_execute completed");
                                serde_json::to_string(r).map_err(|e| anyhow!("failed to serialize mcp result: {}", e))?
                            }
                            Err(e) => {
                                warn!(tool_name = %call.name, error = %e, "tool_execute failed");
                                format!("error: {}", e)
                            }
                        }
                    }
                }
            }
            Err(err) => {
                warn!(tool_name = %call.name, error = %err, "tool_execute failed");
                format!("error: {}", err)
            }
        };

        Ok(output)
    }
}
