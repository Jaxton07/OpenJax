use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use tokio::process::Command;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ApprovalPolicy {
    AlwaysAsk,
    OnRequest,
    Never,
}

impl ApprovalPolicy {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_APPROVAL_POLICY")
            .unwrap_or_else(|_| "always_ask".to_string())
            .as_str()
        {
            "never" => Self::Never,
            "on_request" => Self::OnRequest,
            _ => Self::AlwaysAsk,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlwaysAsk => "always_ask",
            Self::OnRequest => "on_request",
            Self::Never => "never",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SandboxMode {
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_MODE")
            .unwrap_or_else(|_| "workspace_write".to_string())
            .as_str()
        {
            "danger_full_access" => Self::DangerFullAccess,
            _ => Self::WorkspaceWrite,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkspaceWrite => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolRuntimeConfig {
    pub approval_policy: ApprovalPolicy,
    pub sandbox_mode: SandboxMode,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRouter;

impl ToolRouter {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        cwd: &Path,
        config: ToolRuntimeConfig,
    ) -> Result<String> {
        match call.name.as_str() {
            "read_file" => read_file(call, cwd).await,
            "list_dir" => list_dir(call, cwd).await,
            "grep_files" => grep_files(call, cwd).await,
            "exec_command" => exec_command(call, cwd, config).await,
            _ => Err(anyhow!("unknown tool: {}", call.name)),
        }
    }
}

pub fn parse_tool_call(input: &str) -> Option<ToolCall> {
    let trimmed = input.trim();
    let payload = trimmed.strip_prefix("tool:")?;
    let tokens = shlex::split(payload)?;

    let (name, rest) = tokens.split_first()?;
    if name.trim().is_empty() {
        return None;
    }

    let mut args = HashMap::new();
    for token in rest {
        if let Some((k, v)) = token.split_once('=') {
            args.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    Some(ToolCall {
        name: name.trim().to_string(),
        args,
    })
}

async fn read_file(call: &ToolCall, cwd: &Path) -> Result<String> {
    let rel_path = call
        .args
        .get("path")
        .ok_or_else(|| anyhow!("read_file requires path=<relative_path>"))?;

    let path = resolve_workspace_path(cwd, rel_path)?;
    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    Ok(content)
}

async fn list_dir(call: &ToolCall, cwd: &Path) -> Result<String> {
    let rel_path = call
        .args
        .get("path")
        .map_or_else(|| ".".to_string(), Clone::clone);
    let path = resolve_workspace_path(cwd, &rel_path)?;

    let mut entries = tokio::fs::read_dir(&path)
        .await
        .with_context(|| format!("failed to read dir: {}", path.display()))?;

    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        names.push(name.to_string_lossy().to_string());
    }

    names.sort();
    Ok(names.join("\n"))
}

async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String> {
    let pattern = call
        .args
        .get("pattern")
        .ok_or_else(|| anyhow!("grep_files requires pattern=<text>"))?;
    let rel_path = call
        .args
        .get("path")
        .map_or_else(|| ".".to_string(), Clone::clone);
    let root = resolve_workspace_path(cwd, &rel_path)?;

    let mut matches = Vec::new();

    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path: PathBuf = entry.path().to_path_buf();
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            continue;
        };

        for (idx, line) in content.lines().enumerate() {
            if line.contains(pattern) {
                let rel = path.strip_prefix(cwd).unwrap_or(path.as_path());
                matches.push(format!("{}:{}:{}", rel.display(), idx + 1, line));
            }
        }
    }

    if matches.is_empty() {
        Ok("(no matches)".to_string())
    } else {
        Ok(matches.join("\n"))
    }
}

async fn exec_command(call: &ToolCall, cwd: &Path, config: ToolRuntimeConfig) -> Result<String> {
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

    if should_prompt_approval(config.approval_policy, require_escalated)
        && !ask_for_approval(&command)?
    {
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

    Ok(format!(
        "exit_code={exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    ))
}

fn should_prompt_approval(policy: ApprovalPolicy, require_escalated: bool) -> bool {
    match policy {
        ApprovalPolicy::AlwaysAsk => true,
        ApprovalPolicy::OnRequest => require_escalated,
        ApprovalPolicy::Never => false,
    }
}

fn ask_for_approval(command: &str) -> Result<bool> {
    println!("[approval] 执行命令需要确认: {command}");
    println!("[approval] 输入 y 同意，其他任意输入拒绝:");

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("failed to read approval input")?;

    Ok(answer.trim().eq_ignore_ascii_case("y"))
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

fn resolve_workspace_path(cwd: &Path, rel_path: &str) -> Result<PathBuf> {
    let input = Path::new(rel_path);
    if input.is_absolute() {
        return Err(anyhow!(
            "path escapes workspace: absolute paths are not allowed ({})",
            rel_path
        ));
    }

    if contains_parent_dir(input) {
        return Err(anyhow!(
            "path escapes workspace: parent traversal is not allowed ({})",
            rel_path
        ));
    }

    let workspace_root = cwd
        .canonicalize()
        .with_context(|| format!("failed to canonicalize workspace root: {}", cwd.display()))?;
    let resolved = cwd
        .join(input)
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path: {}", cwd.join(input).display()))?;

    if !resolved.starts_with(&workspace_root) {
        return Err(anyhow!(
            "path escapes workspace: {}",
            cwd.join(input).display()
        ));
    }

    Ok(resolved)
}

fn contains_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
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

    if contains_parent_dir(path) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!("openjax-tools-test-{nanos}"))
    }

    fn create_workspace() -> PathBuf {
        let path = temp_workspace_path();
        fs::create_dir_all(&path).expect("failed to create temp workspace");
        path
    }

    #[test]
    fn resolve_workspace_path_rejects_absolute_path() {
        let cwd = create_workspace();
        let result = resolve_workspace_path(&cwd, "/etc/hosts");
        assert!(result.is_err());
        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn resolve_workspace_path_rejects_parent_traversal() {
        let cwd = create_workspace();
        let result = resolve_workspace_path(&cwd, "../outside");
        assert!(result.is_err());
        let _ = fs::remove_dir_all(cwd);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_workspace_path_rejects_symlink_escape() {
        let cwd = create_workspace();
        let link = cwd.join("link_to_etc");
        symlink("/etc", &link).expect("failed to create symlink");

        let result = resolve_workspace_path(&cwd, "link_to_etc/hosts");
        assert!(result.is_err());

        let _ = fs::remove_file(link);
        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn workspace_write_blocks_network_command() {
        let cwd = create_workspace();
        let result = deny_if_blocked_in_workspace_write("curl https://example.com", &cwd);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn workspace_write_blocks_shell_operator() {
        let cwd = create_workspace();
        let result = deny_if_blocked_in_workspace_write("echo hi > /tmp/file.txt", &cwd);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn workspace_write_blocks_parent_path_arg() {
        let cwd = create_workspace();
        let result = deny_if_blocked_in_workspace_write("cat ../secret.txt", &cwd);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn workspace_write_allows_safe_readonly_command() {
        let cwd = create_workspace();
        let file = cwd.join("note.txt");
        fs::write(&file, "hello").expect("failed to write sample file");

        let result = deny_if_blocked_in_workspace_write("cat note.txt", &cwd);
        assert!(result.is_ok());
        let _ = fs::remove_dir_all(cwd);
    }
}
