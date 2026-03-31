use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

use super::de_helpers;
use crate::sandbox;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

#[derive(Deserialize)]
struct ShellCommandArgs {
    cmd: String,
    #[serde(default = "shell_default_timeout", deserialize_with = "de_helpers::de_u64")]
    timeout_ms: u64,
}

fn shell_default_timeout() -> u64 {
    30_000
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

        if invocation.turn.prevent_shell_skill_trigger
            && looks_like_skill_trigger_shell_command(&command)
        {
            let guidance = "selected skill should be executed as workflow steps, not as a shell executable trigger string";
            let output = format!(
                "result_class=failure\ncommand={}\nexit_code=127\nbackend=none\n\
                 degrade_reason=none\npolicy_decision=Allow\nruntime_allowed=false\n\
                 runtime_deny_reason=skill_trigger_not_shell_command\n\
                 guidance={}\nstdout:\n\nstderr:\n{}",
                command, guidance, guidance
            );
            return Ok(ToolOutput::Function {
                body: crate::tools::context::FunctionCallOutputBody::Text(output),
                success: Some(false),
            });
        }

        sandbox::execute_shell(&invocation, &command, timeout_ms).await
    }
}

fn looks_like_skill_trigger_shell_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') {
        return false;
    }
    if trimmed.contains(char::is_whitespace) {
        return false;
    }
    // Treat single-segment slash commands like `/local-commit` as likely skill triggers.
    // Absolute paths such as `/usr/bin/git` still pass through.
    trimmed[1..].chars().all(|ch| ch != '/')
}

#[cfg(test)]
mod tests {
    use super::looks_like_skill_trigger_shell_command;

    #[test]
    fn detects_skill_trigger_like_slash_command() {
        assert!(looks_like_skill_trigger_shell_command("/local-commit"));
        assert!(looks_like_skill_trigger_shell_command("/skill-abc"));
        assert!(!looks_like_skill_trigger_shell_command("/usr/bin/git"));
        assert!(!looks_like_skill_trigger_shell_command("git status"));
    }
}
