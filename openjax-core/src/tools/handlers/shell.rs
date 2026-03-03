use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

use crate::sandbox;
use crate::tools::apply_patch_interceptor;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

#[derive(Deserialize)]
struct ShellCommandArgs {
    cmd: String,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    require_escalated: bool,
    #[serde(default = "shell_default_timeout")]
    timeout_ms: u64,
}

fn shell_default_timeout() -> u64 {
    30_000
}

fn deserialize_boolish<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(v) = value.as_bool() {
        return Ok(v);
    }
    if let Some(v) = value.as_str() {
        return Ok(matches!(
            v.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ));
    }
    Ok(false)
}

pub struct ShellCommandHandler;

#[async_trait]
impl ToolHandler for ShellCommandHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments.clone(),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "shell handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ShellCommandArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
        })?;

        let command = args.cmd;
        let timeout_ms = args.timeout_ms;
        let _require_escalated = args.require_escalated;

        let command_tokens: Vec<String> =
            command.split_whitespace().map(|s| s.to_string()).collect();
        if let Some(output) = apply_patch_interceptor::intercept_apply_patch(
            &command_tokens,
            &invocation.turn.cwd,
            None,
            &invocation.turn,
            &invocation.call_id,
            &invocation.tool_name,
        )
        .await?
        {
            return Ok(ToolOutput::Function {
                body: crate::tools::context::FunctionCallOutputBody::Text(output),
                success: Some(true),
            });
        }

        sandbox::execute_shell(&invocation, &command, timeout_ms).await
    }
}
