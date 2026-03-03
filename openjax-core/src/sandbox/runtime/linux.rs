#[cfg(target_os = "linux")]
use tokio::process::Command;
#[cfg(target_os = "linux")]
use tokio::time::{Duration, timeout};

#[cfg(target_os = "linux")]
use crate::sandbox::policy::{SandboxBackend, SandboxCapability};

#[cfg(not(target_os = "linux"))]
use super::{SandboxExecutionRequest, SandboxExecutionResult};
#[cfg(target_os = "linux")]
use super::{SandboxExecutionRequest, SandboxExecutionResult, wrap_command_for_shell};

pub(super) async fn execute_linux_native(
    request: &SandboxExecutionRequest,
) -> Result<SandboxExecutionResult, String> {
    #[cfg(target_os = "linux")]
    {
        let bwrap = which::which("bwrap")
            .map_err(|_| "bwrap is required for linux_native backend but not found".to_string())?;
        let wrapped_command = wrap_command_for_shell(&request.shell, &request.command);
        let shell_args = request
            .shell
            .derive_exec_args(&wrapped_command, Some(false));
        let mut args: Vec<String> = vec![
            "--die-with-parent".to_string(),
            "--unshare-pid".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--ro-bind".to_string(),
            "/".to_string(),
            "/".to_string(),
            "--bind".to_string(),
            request.cwd.display().to_string(),
            request.cwd.display().to_string(),
            "--chdir".to_string(),
            request.cwd.display().to_string(),
        ];
        if !request.capabilities.contains(&SandboxCapability::Network) {
            args.push("--unshare-net".to_string());
        }
        args.push(request.shell.shell_path.display().to_string());
        args.extend(shell_args);

        let output = timeout(
            Duration::from_millis(request.timeout_ms),
            Command::new(bwrap).args(&args).output(),
        )
        .await
        .map_err(|_| format!("command timed out after {}ms", request.timeout_ms))?
        .map_err(|e| format!("failed to execute bwrap command: {e}"))?;
        let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.code().unwrap_or(-1) != 0
            && looks_like_linux_sandbox_bootstrap_error(&stderr_text)
        {
            return Err(format!("linux_native backend setup failed: {stderr_text}"));
        }

        Ok(SandboxExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: stderr_text,
            backend_used: SandboxBackend::LinuxNative,
            degrade_reason: None,
            policy_trace: request.policy_trace.clone(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = request;
        Err("linux_native backend is unavailable on this platform".to_string())
    }
}

#[cfg(target_os = "linux")]
fn looks_like_linux_sandbox_bootstrap_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("operation not permitted")
        || lower.contains("creating new namespace")
        || lower.contains("unshare")
        || lower.contains("user namespace")
}
