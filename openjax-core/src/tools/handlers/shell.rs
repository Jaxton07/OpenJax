use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use shlex;
use std::path::Path;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::{info, warn};

use crate::approval::ApprovalRequest;
use crate::tools::apply_patch_interceptor;
use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::shell::Shell;
use crate::tools::{ApprovalPolicy, SandboxMode, ShellType};

#[derive(Deserialize)]
struct ShellCommandArgs {
    cmd: String,
    #[serde(default)]
    require_escalated: bool,
    #[serde(default = "shell_default_timeout")]
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
        let ToolInvocation {
            payload,
            turn,
            call_id,
            tool_name,
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
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
        let require_escalated = args.require_escalated;
        let timeout_ms = args.timeout_ms;

        let sandbox_policy = match turn.sandbox_policy {
            crate::tools::SandboxPolicy::None => SandboxMode::DangerFullAccess,
            crate::tools::SandboxPolicy::ReadOnly => SandboxMode::WorkspaceWrite,
            crate::tools::SandboxPolicy::Write => SandboxMode::WorkspaceWrite,
            crate::tools::SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
        };

        let approval_policy = turn.approval_policy;

        info!(
            command = %command,
            require_escalated = require_escalated,
            sandbox_mode = sandbox_policy.as_str(),
            "shell started"
        );

        if should_prompt_approval(approval_policy, require_escalated) {
            let approved = turn
                .approval_handler
                .request_approval(ApprovalRequest {
                    target: command.clone(),
                    reason: if require_escalated {
                        "shell command requested escalated permissions".to_string()
                    } else {
                        "shell command requires approval by policy".to_string()
                    },
                })
                .await
                .map_err(FunctionCallError::Internal)?;

            if !approved {
                warn!(command = %command, "shell rejected by user");
                return Err(FunctionCallError::ApprovalRejected(
                    "command rejected by user".to_string(),
                ));
            }
        }

        if let SandboxMode::WorkspaceWrite = sandbox_policy {
            deny_if_blocked_in_workspace_write(&command, &turn.cwd)
                .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
        }

        let command_tokens: Vec<String> =
            command.split_whitespace().map(|s| s.to_string()).collect();

        if let Some(output) = apply_patch_interceptor::intercept_apply_patch(
            &command_tokens,
            &turn.cwd,
            None,
            &turn,
            &call_id,
            &tool_name,
        )
        .await?
        {
            return Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text(output),
                success: Some(true),
            });
        }

        let shell = Shell::new(ShellType::default())
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;
        let shell_args = shell.derive_exec_args(&command, None);

        let output = timeout(
            Duration::from_millis(timeout_ms),
            Command::new(&shell.shell_path)
                .args(&shell_args)
                .current_dir(&turn.cwd)
                .output(),
        )
        .await
        .map_err(|_| {
            FunctionCallError::Internal(format!("command timed out after {}ms", timeout_ms))
        })?
        .map_err(|e| FunctionCallError::Internal(format!("failed to execute command: {}", e)))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        info!(
            command = %command,
            exit_code = exit_code,
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            "shell completed"
        );

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(format!(
                "exit_code={exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
            )),
            success: Some(exit_code == 0),
        })
    }
}

fn should_prompt_approval(policy: ApprovalPolicy, require_escalated: bool) -> bool {
    match policy {
        ApprovalPolicy::AlwaysAsk => true,
        ApprovalPolicy::OnRequest => require_escalated,
        ApprovalPolicy::Never => false,
    }
}

fn deny_if_blocked_in_workspace_write(command: &str, cwd: &Path) -> Result<()> {
    let lower = command.to_ascii_lowercase();
    let blocked_keywords = [
        "curl ", "wget ", "ssh ", "scp ", "nc ", "nmap ", "ping ", "sudo ",
    ];

    if blocked_keywords.iter().any(|kw| lower.contains(kw)) {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: network/escalation command detected"
        ));
    }

    if lower.contains("rm -rf /") {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: destructive root delete detected"
        ));
    }

    let blocked_shell_operators = ["&&", "||", "|", ";", ">", "<", "`", "$("];
    if blocked_shell_operators
        .iter()
        .any(|operator| command.contains(operator))
    {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: shell operators are not allowed"
        ));
    }

    let tokens = shlex::split(command).ok_or_else(|| {
        anyhow!("command blocked by workspace_write sandbox policy: invalid shell command syntax")
    })?;
    if tokens.is_empty() {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: empty command"
        ));
    }

    let allowed_programs = [
        "pwd", "ls", "cat", "rg", "grep", "find", "head", "tail", "wc", "sed", "awk", "echo",
        "stat", "uname", "which", "env", "printf",
    ];
    let program = tokens[0].to_ascii_lowercase();
    if !allowed_programs.contains(&program.as_str()) {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: command `{}` is not in allowlist",
            tokens[0]
        ));
    }

    for arg in tokens.iter().skip(1) {
        if arg.starts_with('-') || !looks_like_path_arg(arg) {
            continue;
        }
        validate_command_path_arg(arg, cwd)?;
    }

    Ok(())
}

fn looks_like_path_arg(arg: &str) -> bool {
    arg == "."
        || arg == ".."
        || arg.starts_with("./")
        || arg.starts_with("../")
        || arg.starts_with('/')
        || arg.starts_with("~/")
        || arg.contains('/')
}

fn validate_command_path_arg(arg: &str, cwd: &Path) -> Result<()> {
    if arg.starts_with("~/") {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: home directory paths are not allowed ({arg})"
        ));
    }

    let path = Path::new(arg);
    if path.is_absolute() {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: absolute paths are not allowed ({arg})"
        ));
    }

    if crate::tools::contains_parent_dir(path) {
        return Err(anyhow!(
            "command blocked by workspace_write sandbox policy: parent traversal is not allowed ({arg})"
        ));
    }

    let joined = cwd.join(path);
    if joined.exists() {
        let workspace_root = cwd
            .canonicalize()
            .with_context(|| format!("failed to canonicalize workspace root: {}", cwd.display()))?;
        let resolved = joined.canonicalize().with_context(|| {
            format!("failed to canonicalize command path: {}", joined.display())
        })?;

        if !resolved.starts_with(&workspace_root) {
            return Err(anyhow!(
                "command blocked by workspace_write sandbox policy: path escapes workspace ({arg})"
            ));
        }
    }

    Ok(())
}
