use anyhow::{Result, anyhow};
use openjax_protocol::ShellExecutionMetadata;
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
    pub model_content: String,
    pub display_output: String,
    pub shell_metadata: Option<ShellExecutionMetadata>,
    pub success: bool,
}

#[derive(Clone)]
pub struct ToolRouter {
    orchestrator: Arc<ToolOrchestrator>,
    specs: Vec<ToolSpec>,
}

pub struct ToolExecutionRequest<'a> {
    pub turn_id: u64,
    pub session_id: Option<String>,
    pub tool_call_id: String,
    pub call: &'a ToolCall,
    pub cwd: &'a std::path::Path,
    pub config: ToolRuntimeConfig,
    pub approval_handler: Arc<dyn ApprovalHandler>,
    pub event_sink: Option<tokio::sync::mpsc::UnboundedSender<openjax_protocol::Event>>,
    pub policy_runtime: Option<openjax_policy::runtime::PolicyRuntime>,
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
        self.specs
            .iter()
            .find(|s| s.name == tool_name)
            .map(|s| s.display_name.clone())
    }

    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    pub async fn execute(&self, request: ToolExecutionRequest<'_>) -> Result<ToolExecOutcome> {
        let ToolExecutionRequest {
            turn_id,
            session_id,
            tool_call_id,
            call,
            cwd,
            config,
            approval_handler,
            event_sink,
            policy_runtime,
        } = request;
        debug!(
            tool_name = %call.name,
            args = ?call.args,
            args_json = %serde_json::to_string(&call.args).unwrap_or_default(),
            cwd = %cwd.display(),
            sandbox_mode = config.sandbox_mode.as_str(),
            "tool_execute started"
        );

        let sandbox_policy = match config.sandbox_mode {
            super::router::SandboxMode::WorkspaceWrite => SandboxPolicy::Write,
            super::router::SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        };

        let invocation = create_tool_invocation(CreateToolInvocationParams {
            turn_id,
            session_id,
            call_id: tool_call_id,
            tool_name: call.name.clone(),
            arguments: serde_json::to_string(&call.args)
                .map_err(|e| anyhow!("failed to serialize args: {}", e))?,
            cwd: cwd.to_path_buf(),
            sandbox_policy,
            shell_type: config.shell_type,
            prevent_shell_skill_trigger: config.prevent_shell_skill_trigger,
            approval_handler,
            event_sink,
            policy_runtime,
        });

        let result = self.orchestrator.run(invocation).await;

        match result {
            Ok(tool_output) => match tool_output {
                super::context::ToolOutput::Function { body, success } => match body {
                    super::context::FunctionCallOutputBody::Text(text) => {
                        let success = success.unwrap_or(true);
                        let (model_content, display_output, shell_metadata) =
                            normalize_tool_output(&call.name, text);
                        let preview = summarize_preview(&display_output, 240);
                        info!(
                            tool_name = %call.name,
                            success = success,
                            output_len = display_output.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            model_content,
                            display_output,
                            shell_metadata,
                            success,
                        })
                    }
                    super::context::FunctionCallOutputBody::Json(json) => {
                        let success = success.unwrap_or(true);
                        let text = json.to_string();
                        let (model_content, display_output, shell_metadata) =
                            normalize_tool_output(&call.name, text);
                        let preview = summarize_preview(&display_output, 240);
                        info!(
                            tool_name = %call.name,
                            success = success,
                            output_len = display_output.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            model_content,
                            display_output,
                            shell_metadata,
                            success,
                        })
                    }
                },
                super::context::ToolOutput::Mcp { result, .. } => match result {
                    Ok(r) => {
                        let text = serde_json::to_string(&r)
                            .map_err(|e| anyhow!("failed to serialize mcp result: {}", e))?;
                        let (model_content, display_output, shell_metadata) =
                            normalize_tool_output(&call.name, text);
                        let preview = summarize_preview(&display_output, 240);
                        info!(
                            tool_name = %call.name,
                            success = true,
                            output_len = display_output.len(),
                            output_preview = %preview,
                            "tool_execute completed"
                        );
                        Ok(ToolExecOutcome {
                            model_content,
                            display_output,
                            shell_metadata,
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

fn normalize_tool_output(
    tool_name: &str,
    display_output: String,
) -> (String, String, Option<ShellExecutionMetadata>) {
    if is_shell_tool(tool_name)
        && let Some((model_content, shell_metadata)) = parse_shell_display_output(&display_output)
    {
        return (model_content, display_output, Some(shell_metadata));
    }
    (display_output.clone(), display_output, None)
}

fn is_shell_tool(tool_name: &str) -> bool {
    matches!(tool_name, "shell" | "exec_command")
}

fn parse_shell_display_output(display_output: &str) -> Option<(String, ShellExecutionMetadata)> {
    let (header, body) = display_output.split_once("\nstdout:\n")?;
    let (stdout, stderr) = body.split_once("\nstderr:\n")?;

    let mut result_class: Option<String> = None;
    let mut backend: Option<String> = None;
    let mut exit_code: Option<i32> = None;
    let mut policy_decision: Option<String> = None;
    let mut runtime_allowed: Option<bool> = None;
    let mut degrade_reason: Option<String> = None;
    let mut runtime_deny_reason: Option<String> = None;

    for line in header.lines() {
        if let Some(v) = line.strip_prefix("result_class=") {
            result_class = Some(v.to_string());
            continue;
        }
        if let Some(v) = line.strip_prefix("backend=") {
            backend = Some(v.to_string());
            continue;
        }
        if let Some(v) = line.strip_prefix("exit_code=") {
            exit_code = v.parse::<i32>().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("policy_decision=") {
            policy_decision = Some(v.to_string());
            continue;
        }
        if let Some(v) = line.strip_prefix("runtime_allowed=") {
            runtime_allowed = v.parse::<bool>().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("degrade_reason=") {
            degrade_reason = optional_shell_field(v);
            continue;
        }
        if let Some(v) = line.strip_prefix("runtime_deny_reason=") {
            runtime_deny_reason = optional_shell_field(v);
        }
    }

    let exit_code = exit_code?;
    let model_content = build_shell_model_content(exit_code, stdout, stderr);
    let shell_metadata = ShellExecutionMetadata {
        result_class: result_class?,
        backend: backend?,
        exit_code,
        policy_decision: policy_decision?,
        runtime_allowed: runtime_allowed?,
        degrade_reason,
        runtime_deny_reason,
    };
    Some((model_content, shell_metadata))
}

fn optional_shell_field(value: &str) -> Option<String> {
    if value.is_empty() || value == "none" {
        return None;
    }
    Some(value.to_string())
}

fn build_shell_model_content(exit_code: i32, stdout: &str, stderr: &str) -> String {
    let mut model_content = format!("exit_code={exit_code}\nstdout:\n{stdout}");
    if !stderr.is_empty() {
        model_content.push_str("\nstderr:\n");
        model_content.push_str(stderr);
    }
    model_content
}
