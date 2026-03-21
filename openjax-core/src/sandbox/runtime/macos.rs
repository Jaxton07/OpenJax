#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use tokio::process::Command;
#[cfg(target_os = "macos")]
use tokio::time::{Duration, timeout};
#[cfg(target_os = "macos")]
use tracing::info;

#[cfg(target_os = "macos")]
use crate::sandbox::policy::SandboxBackend;

#[cfg(not(target_os = "macos"))]
use super::{SandboxExecutionRequest, SandboxExecutionResult};
#[cfg(target_os = "macos")]
use super::{
    SandboxExecutionRequest, SandboxExecutionResult, summarize_preview, wrap_command_for_runner,
};

pub(super) async fn execute_macos_seatbelt(
    request: &SandboxExecutionRequest,
) -> Result<SandboxExecutionResult, String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(health_err) = macos_seatbelt_unavailable_reason() {
            return Err(format!("macos_seatbelt_unavailable_cached: {health_err}"));
        }
        let sandbox_exec = which::which("sandbox-exec").map_err(|_| {
            "macos_seatbelt_missing_binary: sandbox-exec is required for macos_seatbelt backend but not found".to_string()
        })?;
        let profile = render_macos_profile(
            &request.cwd,
            request
                .capabilities
                .iter()
                .any(|cap| matches!(cap, crate::sandbox::policy::SandboxCapability::Network)),
        );
        let mut args = vec!["-p".to_string(), profile];
        let seatbelt_runner = "/bin/sh".to_string();
        let wrapped_command = wrap_command_for_runner(&seatbelt_runner, &request.command);
        let runner_args = vec!["-c".to_string(), wrapped_command];
        args.push(seatbelt_runner.clone());
        args.extend(runner_args.clone());
        info!(
            command = %request.command,
            seatbelt_runner = %seatbelt_runner,
            shell_args = ?runner_args,
            "macos seatbelt executing command"
        );

        let output = timeout(
            Duration::from_millis(request.timeout_ms),
            Command::new(sandbox_exec).args(&args).output(),
        )
        .await
        .map_err(|_| {
            format!(
                "macos_seatbelt_timeout: command timed out after {}ms",
                request.timeout_ms
            )
        })?
        .map_err(|e| {
            format!("macos_seatbelt_spawn_failed: failed to execute sandbox-exec command: {e}")
        })?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if exit_code != 0 && stdout.trim().is_empty() && stderr.trim().is_empty() {
            info!(
                command = %request.command,
                exit_code = exit_code,
                seatbelt_runner = %seatbelt_runner,
                profile_preview = %summarize_preview(&args[1], 180),
                shell_args = ?&args[2..],
                "macos seatbelt returned non-zero without stderr/stdout"
            );
            return Err(format!(
                "macos_seatbelt_unknown_nonzero: backend returned non-zero exit without stderr/stdout (exit_code={exit_code})"
            ));
        }
        if exit_code != 0 {
            info!(
                command = %request.command,
                exit_code = exit_code,
                seatbelt_runner = %seatbelt_runner,
                stderr_preview = %summarize_preview(&stderr, 220),
                "macos seatbelt returned non-zero with stderr"
            );
            return Err(classify_macos_seatbelt_error(&stderr));
        }

        Ok(SandboxExecutionResult {
            exit_code,
            stdout,
            stderr,
            backend_used: SandboxBackend::MacosSeatbelt,
            degrade_reason: None,
            policy_trace: request.policy_trace.clone(),
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        Err("macos_seatbelt backend is unavailable on this platform".to_string())
    }
}

#[cfg(target_os = "macos")]
fn macos_seatbelt_unavailable_reason() -> Option<String> {
    static HEALTH: OnceLock<Option<String>> = OnceLock::new();
    let reason = HEALTH.get_or_init(probe_macos_seatbelt_health).clone();
    if let Some(r) = &reason {
        info!(reason = %r, "macos seatbelt health cache unavailable");
    }
    reason
}

#[cfg(target_os = "macos")]
fn probe_macos_seatbelt_health() -> Option<String> {
    let sandbox_exec = match which::which("sandbox-exec") {
        Ok(path) => path,
        Err(_) => {
            return Some(
                "macos_seatbelt_missing_binary: sandbox-exec is not available in PATH".to_string(),
            );
        }
    };
    let output = match std::process::Command::new(sandbox_exec)
        .args([
            "-p",
            "(version 1) (deny default) (allow process*) (allow file-read*)",
            "/bin/sh",
            "-c",
            "true",
        ])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return Some(format!(
                "macos_seatbelt_probe_spawn_failed: failed to run sandbox-exec probe: {err}"
            ));
        }
    };
    if output.status.success() {
        info!(
            probe_runner = "/bin/sh -c true",
            "macos seatbelt probe passed"
        );
        return None;
    }
    let probe_exit = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    info!(
        probe_runner = "/bin/sh -c true",
        probe_exit = probe_exit,
        probe_stderr_preview = %summarize_preview(&stderr, 220),
        "macos seatbelt probe failed"
    );
    Some(format!(
        "{}; probe_runner=/bin/sh -c true; probe_exit={}",
        classify_macos_seatbelt_error(&stderr),
        probe_exit
    ))
}

#[cfg(target_os = "macos")]
fn classify_macos_seatbelt_error(stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    let normalized = stderr.trim();
    if lower.contains("sandbox_apply") && lower.contains("operation not permitted") {
        return format!(
            "macos_seatbelt_apply_denied: sandbox apply not permitted by current runtime ({})",
            normalized
        );
    }
    if lower.contains("invalid profile") || lower.contains("parse error") {
        return format!(
            "macos_seatbelt_profile_invalid: sandbox profile rejected ({})",
            normalized
        );
    }
    if lower.contains("operation not permitted") {
        return format!(
            "macos_seatbelt_permission_denied: sandbox execution denied ({})",
            normalized
        );
    }
    format!("macos_seatbelt_runtime_error: {}", normalized)
}

#[cfg(target_os = "macos")]
fn render_macos_profile(cwd: &std::path::Path, allow_network: bool) -> String {
    let cwd_literal = cwd.display().to_string().replace('\"', "\\\"");
    let mut profile = format!(
        "(version 1)\n(deny default)\n(allow process*)\n(allow file-read*)\n(allow file-write* (subpath \"{cwd_literal}\"))\n(allow file-read* (subpath \"{cwd_literal}\"))"
    );
    if allow_network {
        profile.push_str("\n(allow network*)");
    }
    profile
}
