use anyhow::{Context, Result, anyhow};
use std::path::Path;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::{info, warn};

use crate::tools::{ToolCall, ToolRuntimeConfig, ApprovalPolicy, SandboxMode};

pub async fn exec_command(call: &ToolCall, cwd: &Path, config: ToolRuntimeConfig) -> Result<String> {
    let command = call
        .args
        .get("cmd")
        .ok_or_else(|| anyhow!("exec_command requires cmd='<shell command>'"))?
        .to_string();

    let require_escalated = call
        .args
        .get("require_escalated")
        .map(|value| value == "true")
        .unwrap_or(false);

    info!(
        command = %command,
        require_escalated = require_escalated,
        sandbox_mode = config.sandbox_mode.as_str(),
        "exec_command started"
    );

    if should_prompt_approval(config.approval_policy, require_escalated)
        && !ask_for_approval(&command)?
    {
        warn!(command = %command, "exec_command rejected by user");
        return Err(anyhow!("command rejected by user"));
    }

    if let SandboxMode::WorkspaceWrite = config.sandbox_mode {
        deny_if_blocked_in_workspace_write(&command, cwd)?;
    }

    let timeout_ms = call
        .args
        .get("timeout_ms")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30_000);

    let output = timeout(
        Duration::from_millis(timeout_ms),
        Command::new("zsh")
            .arg("-lc")
            .arg(&command)
            .current_dir(cwd)
            .output(),
    )
    .await
    .map_err(|_| anyhow!("command timed out after {timeout_ms}ms"))??;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    info!(
        command = %command,
        exit_code = exit_code,
        stdout_len = stdout.len(),
        stderr_len = stderr.len(),
        "exec_command completed"
    );

    Ok(format!(
        "exit_code={exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    ))
}

pub fn should_prompt_approval(policy: ApprovalPolicy, require_escalated: bool) -> bool {
    match policy {
        ApprovalPolicy::AlwaysAsk => true,
        ApprovalPolicy::OnRequest => require_escalated,
        ApprovalPolicy::Never => false,
    }
}

pub fn ask_for_approval(command: &str) -> Result<bool> {
    println!("[approval] 执行命令需要确认: {command}");
    println!("[approval] 输入 y 同意，其他任意输入拒绝:");

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("failed to read approval input")?;

    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

pub fn deny_if_blocked_in_workspace_write(command: &str, cwd: &Path) -> Result<()> {
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
