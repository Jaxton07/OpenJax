use anyhow::{Result, anyhow};
use tokio::process::Command;
use tokio::time::{Duration, timeout};

use super::{SandboxExecutionRequest, wrap_command_for_shell};

pub async fn run_without_sandbox(
    request: &SandboxExecutionRequest,
) -> Result<(i32, String, String)> {
    let wrapped_command = wrap_command_for_shell(&request.shell, &request.command);
    let shell_args = request
        .shell
        .derive_exec_args(&wrapped_command, Some(false));
    let output = timeout(
        Duration::from_millis(request.timeout_ms),
        Command::new(&request.shell.shell_path)
            .args(&shell_args)
            .current_dir(&request.cwd)
            .output(),
    )
    .await
    .map_err(|_| anyhow!("command timed out after {}ms", request.timeout_ms))?
    .map_err(|e| anyhow!("failed to execute command: {e}"))?;

    Ok((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}
