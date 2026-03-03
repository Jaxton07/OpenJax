use anyhow::{Context, Result, anyhow};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::info;

use crate::tools::policy::{PolicyTrace, SandboxBackend, SandboxCapability};
use crate::tools::shell::{Shell, ShellType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxBackendPreference {
    Auto,
    LinuxNative,
    MacosSeatbelt,
    None,
}

impl SandboxBackendPreference {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_BACKEND")
            .unwrap_or_else(|_| "auto".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "linux_native" => Self::LinuxNative,
            "macos_seatbelt" => Self::MacosSeatbelt,
            "none" => Self::None,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxDegradePolicy {
    AskThenAllow,
    Deny,
}

impl SandboxDegradePolicy {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_DEGRADE_POLICY")
            .unwrap_or_else(|_| "ask_then_allow".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "deny" => Self::Deny,
            _ => Self::AskThenAllow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxRuntimeSettings {
    pub backend_preference: SandboxBackendPreference,
    pub degrade_policy: SandboxDegradePolicy,
    pub audit_enabled: bool,
}

impl SandboxRuntimeSettings {
    pub fn from_env() -> Self {
        let audit_enabled = std::env::var("OPENJAX_SANDBOX_AUDIT")
            .map(|v| !matches!(v.as_str(), "0" | "false" | "FALSE"))
            .unwrap_or(true);
        Self {
            backend_preference: SandboxBackendPreference::from_env(),
            degrade_policy: SandboxDegradePolicy::from_env(),
            audit_enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxExecutionRequest {
    pub command: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
    pub capabilities: Vec<SandboxCapability>,
    pub shell: Shell,
    pub policy_trace: PolicyTrace,
    pub preferred_backend: Option<SandboxBackend>,
}

#[derive(Debug, Clone)]
pub struct SandboxExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub backend_used: SandboxBackend,
    pub degrade_reason: Option<String>,
    pub policy_trace: PolicyTrace,
}

#[derive(Debug, Clone)]
pub struct BackendUnavailable {
    pub backend: SandboxBackend,
    pub reason: String,
}

pub async fn execute_in_sandbox(
    request: &SandboxExecutionRequest,
    settings: SandboxRuntimeSettings,
) -> Result<SandboxExecutionResult, BackendUnavailable> {
    let selected = select_backend(request, settings.backend_preference);
    match selected {
        SandboxBackend::NoneEscalated => run_without_sandbox(request)
            .await
            .map(|(code, stdout, stderr)| SandboxExecutionResult {
                exit_code: code,
                stdout,
                stderr,
                backend_used: SandboxBackend::NoneEscalated,
                degrade_reason: None,
                policy_trace: request.policy_trace.clone(),
            })
            .map_err(|err| BackendUnavailable {
                backend: SandboxBackend::NoneEscalated,
                reason: err.to_string(),
            }),
        SandboxBackend::LinuxNative => {
            execute_linux_native(request)
                .await
                .map_err(|reason| BackendUnavailable {
                    backend: SandboxBackend::LinuxNative,
                    reason,
                })
        }
        SandboxBackend::MacosSeatbelt => {
            execute_macos_seatbelt(request)
                .await
                .map_err(|reason| BackendUnavailable {
                    backend: SandboxBackend::MacosSeatbelt,
                    reason,
                })
        }
    }
}

fn select_backend(
    request: &SandboxExecutionRequest,
    preference: SandboxBackendPreference,
) -> SandboxBackend {
    if let Some(explicit) = request.preferred_backend {
        return explicit;
    }
    match preference {
        SandboxBackendPreference::LinuxNative => SandboxBackend::LinuxNative,
        SandboxBackendPreference::MacosSeatbelt => SandboxBackend::MacosSeatbelt,
        SandboxBackendPreference::None => SandboxBackend::NoneEscalated,
        SandboxBackendPreference::Auto => {
            #[cfg(target_os = "linux")]
            {
                SandboxBackend::LinuxNative
            }
            #[cfg(target_os = "macos")]
            {
                SandboxBackend::MacosSeatbelt
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                SandboxBackend::NoneEscalated
            }
        }
    }
}

async fn execute_linux_native(
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

async fn execute_macos_seatbelt(
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
                .any(|cap| matches!(cap, SandboxCapability::Network)),
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
        .map_err(|_| format!("macos_seatbelt_timeout: command timed out after {}ms", request.timeout_ms))?
        .map_err(|e| format!("macos_seatbelt_spawn_failed: failed to execute sandbox-exec command: {e}"))?;

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
            )
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
            ))
        }
    };
    if output.status.success() {
        info!(probe_runner = "/bin/sh -c true", "macos seatbelt probe passed");
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

fn render_macos_profile(cwd: &Path, allow_network: bool) -> String {
    let cwd_literal = cwd.display().to_string().replace('\"', "\\\"");
    let mut profile = format!(
        "(version 1)\n(deny default)\n(allow process*)\n(allow file-read*)\n(allow file-write* (subpath \"{cwd_literal}\"))\n(allow file-read* (subpath \"{cwd_literal}\"))"
    );
    if allow_network {
        profile.push_str("\n(allow network*)");
    }
    profile
}

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

pub fn fnv1a64(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn summarize_capabilities(caps: &[SandboxCapability]) -> String {
    let mut set = BTreeSet::new();
    for cap in caps {
        set.insert(cap.as_str());
    }
    set.into_iter().collect::<Vec<_>>().join(",")
}

pub fn audit_log(
    settings: SandboxRuntimeSettings,
    request: &SandboxExecutionRequest,
    result: Option<&SandboxExecutionResult>,
    backend_error: Option<&BackendUnavailable>,
) {
    if !settings.audit_enabled {
        return;
    }
    let command_hash = fnv1a64(&request.command);
    let caps = summarize_capabilities(&request.capabilities);
    if let Some(ok) = result {
        info!(
            command_hash = %command_hash,
            backend = ok.backend_used.as_str(),
            capabilities = %caps,
            decision = ?ok.policy_trace.decision,
            degrade_reason = ?ok.degrade_reason,
            "sandbox audit success"
        );
        return;
    }
    if let Some(err) = backend_error {
        info!(
            command_hash = %command_hash,
            backend = err.backend.as_str(),
            capabilities = %caps,
            reason = %err.reason,
            "sandbox audit backend unavailable"
        );
    }
}

pub fn ensure_workspace_relative_paths(command: &str, cwd: &Path) -> Result<()> {
    let tokens = shlex::split(command).ok_or_else(|| anyhow!("invalid shell command syntax"))?;
    for arg in tokens.iter().skip(1) {
        if arg.starts_with('-') || !looks_like_path_arg(arg) {
            continue;
        }
        validate_command_path_arg(arg, cwd)?;
    }
    Ok(())
}

fn wrap_command_for_shell(shell: &Shell, command: &str) -> String {
    match shell.shell_type {
        ShellType::Bash | ShellType::Zsh => format!("set -o pipefail; {command}"),
        ShellType::Sh | ShellType::PowerShell => command.to_string(),
    }
}

fn wrap_command_for_runner(runner: &str, command: &str) -> String {
    if runner.ends_with("/sh") || runner == "sh" {
        // Try enabling pipefail where supported, while staying compatible with strict sh.
        return format!("set -o pipefail >/dev/null 2>&1; {command}");
    }
    command.to_string()
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
        return Err(anyhow!("home directory paths are not allowed ({arg})"));
    }

    let path = Path::new(arg);
    if path.is_absolute() {
        return Err(anyhow!("absolute paths are not allowed ({arg})"));
    }
    if crate::tools::contains_parent_dir(path) {
        return Err(anyhow!("parent traversal is not allowed ({arg})"));
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
            return Err(anyhow!("path escapes workspace ({arg})"));
        }
    }
    Ok(())
}
