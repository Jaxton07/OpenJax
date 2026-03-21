mod linux;
mod macos;
mod none;
mod windows;

use crate::sandbox::policy::{PolicyTrace, SandboxBackend, SandboxCapability};
use crate::tools::shell::{Shell, ShellType};
use anyhow::{Context, Result, anyhow};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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
        SandboxBackend::NoneEscalated => none::run_without_sandbox(request)
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
            linux::execute_linux_native(request)
                .await
                .map_err(|reason| BackendUnavailable {
                    backend: SandboxBackend::LinuxNative,
                    reason,
                })
        }
        SandboxBackend::MacosSeatbelt => {
            macos::execute_macos_seatbelt(request)
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

pub fn ensure_workspace_relative_paths(command: &str, cwd: &Path) -> Result<()> {
    let tokens = shlex::split(command).ok_or_else(|| anyhow!("invalid shell command syntax"))?;
    let mut start_index = 1usize;
    if is_explicit_cwd_chdir_prefix(&tokens, cwd)? {
        start_index = 3;
    }

    for arg in tokens.iter().skip(start_index) {
        if arg.starts_with('-') || !looks_like_path_arg(arg) {
            continue;
        }
        validate_command_path_arg(arg, cwd)?;
    }
    Ok(())
}

pub(crate) fn wrap_command_for_shell(shell: &Shell, command: &str) -> String {
    match shell.shell_type {
        ShellType::Bash | ShellType::Zsh => format!("set -o pipefail; {command}"),
        ShellType::Sh | ShellType::PowerShell => command.to_string(),
    }
}

#[cfg(any(test, target_os = "macos"))]
pub(crate) fn wrap_command_for_runner(runner: &str, command: &str) -> String {
    if runner.ends_with("/sh") || runner == "sh" {
        // Avoid redirection to /dev/null under strict seatbelt profiles; plain sh may not
        // support pipefail anyway, so keep runner command minimal and portable.
        return command.to_string();
    }
    command.to_string()
}

#[cfg(any(test, target_os = "macos"))]
pub(crate) fn summarize_preview(text: &str, limit: usize) -> String {
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

fn is_explicit_cwd_chdir_prefix(tokens: &[String], cwd: &Path) -> Result<bool> {
    if tokens.len() < 3 || tokens[0] != "cd" || tokens[2] != "&&" {
        return Ok(false);
    }

    let target = Path::new(&tokens[1]);
    if !target.is_absolute() {
        return Ok(false);
    }

    let workspace_root = cwd
        .canonicalize()
        .with_context(|| format!("failed to canonicalize workspace root: {}", cwd.display()))?;
    let target_root = target
        .canonicalize()
        .with_context(|| format!("failed to canonicalize chdir target: {}", target.display()))?;

    Ok(target_root == workspace_root)
}

pub use none::run_without_sandbox;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ensure_workspace_relative_paths, wrap_command_for_runner};

    #[test]
    fn sh_runner_does_not_inject_pipefail_with_devnull_redirection() {
        let wrapped = wrap_command_for_runner("/bin/sh", "cat test.txt");
        assert_eq!(wrapped, "cat test.txt");
        assert!(!wrapped.contains("/dev/null"));
    }

    #[test]
    fn allows_absolute_chdir_when_target_is_current_workspace() {
        let cwd = std::env::current_dir().expect("current dir");
        let command = format!("cd {} && git status", cwd.display());
        let result = ensure_workspace_relative_paths(&command, &cwd);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_absolute_chdir_when_target_is_not_workspace() {
        let cwd = std::env::current_dir().expect("current dir");
        let outside = if cfg!(windows) { "C:/Windows" } else { "/tmp" };
        let command = format!("cd {outside} && ls");
        let result = ensure_workspace_relative_paths(&command, Path::new(&cwd));
        assert!(result.is_err());
    }
}
