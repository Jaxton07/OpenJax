use anyhow::{Context, Result, anyhow};
use openjax_protocol::{AgentStatus, ThreadId};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use tokio::process::Command;
use tokio::time::{Duration, timeout};

// ============== Multi-Agent Configuration ==============

/// Maximum depth for sub-agent spawning (from protocol)
pub const MAX_AGENT_DEPTH: i32 = openjax_protocol::MAX_AGENT_DEPTH;

/// Maximum number of concurrent agents per session
pub const DEFAULT_MAX_AGENTS: usize = 4;

/// Agent control configuration (预留扩展)
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum concurrent agents allowed
    pub max_agents: usize,
    /// Maximum depth for sub-agent spawning
    pub max_depth: i32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_agents: DEFAULT_MAX_AGENTS,
            max_depth: MAX_AGENT_DEPTH,
        }
    }
}

/// Agent runtime state (预留扩展)
#[derive(Debug, Clone)]
pub struct AgentRuntime {
    /// Current agent's thread ID
    pub thread_id: ThreadId,
    /// Parent thread ID (None for root agent)
    pub parent_thread_id: Option<ThreadId>,
    /// Agent depth in the hierarchy
    pub depth: i32,
    /// Current status
    pub status: AgentStatus,
}

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
            "apply_patch" => apply_patch_tool(call, cwd).await,
            _ => Err(anyhow!("unknown tool: {}", call.name)),
        }
    }
}

#[derive(Debug, Clone)]
enum PatchOperation {
    AddFile { path: String, lines: Vec<String> },
    DeleteFile { path: String },
    UpdateFile { path: String, hunks: Vec<PatchHunk> },
    MoveFile { from: String, to: String },
    RenameFile { from: String, to: String },
}

#[derive(Debug, Clone)]
struct PatchHunk {
    lines: Vec<PatchHunkLine>,
}

#[derive(Debug, Clone)]
struct PatchHunkLine {
    kind: PatchLineKind,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchLineKind {
    Context,
    Remove,
    Add,
}

#[derive(Debug, Clone)]
enum PlannedAction {
    Create { path: PathBuf, content: String },
    Update { path: PathBuf, content: String },
    Delete { path: PathBuf },
    Move { from: PathBuf, to: PathBuf },
}

impl PlannedAction {
    fn path(&self) -> &Path {
        match self {
            Self::Create { path, .. }
            | Self::Update { path, .. }
            | Self::Delete { path }
            | Self::Move { to: path, .. } => {
                path.as_path()
            }
        }
    }

    fn summary(&self, cwd: &Path) -> String {
        match self {
            Self::Create { path, .. } => format!("ADD {}", display_rel_path(cwd, path)),
            Self::Update { path, .. } => format!("UPDATE {}", display_rel_path(cwd, path)),
            Self::Delete { path } => format!("DELETE {}", display_rel_path(cwd, path)),
            Self::Move { from, to } => {
                format!(
                    "MOVE {} -> {}",
                    display_rel_path(cwd, from),
                    display_rel_path(cwd, to)
                )
            }
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

async fn apply_patch_tool(call: &ToolCall, cwd: &Path) -> Result<String> {
    let patch_arg = call
        .args
        .get("patch")
        .ok_or_else(|| anyhow!("apply_patch requires patch='<patch text>'"))?;
    let normalized_patch = normalize_patch_arg(patch_arg);
    let operations = parse_apply_patch(&normalized_patch)?;
    let actions = plan_patch_actions(cwd, &operations).await?;
    apply_patch_actions(&actions).await?;

    let summary = actions
        .iter()
        .map(|action| action.summary(cwd))
        .collect::<Vec<String>>()
        .join("\n");

    Ok(format!("patch applied successfully\n{summary}"))
}

fn normalize_patch_arg(raw: &str) -> String {
    if raw.contains('\n') {
        raw.to_string()
    } else if raw.contains("\\n") {
        raw.replace("\\n", "\n")
    } else {
        raw.to_string()
    }
}

fn parse_apply_patch(patch: &str) -> Result<Vec<PatchOperation>> {
    let lines = patch.lines().collect::<Vec<&str>>();
    if lines.len() < 2 {
        return Err(anyhow!("invalid patch: too short"));
    }
    if lines[0].trim() != "*** Begin Patch" {
        return Err(anyhow!("invalid patch: missing `*** Begin Patch`"));
    }
    if lines[lines.len() - 1].trim() != "*** End Patch" {
        return Err(anyhow!("invalid patch: missing `*** End Patch`"));
    }

    let mut index = 1usize;
    let mut operations = Vec::new();
    while index < lines.len() - 1 {
        let line = lines[index];
        if line.starts_with("*** Add File: ") {
            let path = line.trim_start_matches("*** Add File: ").trim().to_string();
            if path.is_empty() {
                return Err(anyhow!("invalid patch: empty path in Add File"));
            }
            index += 1;
            let mut add_lines = Vec::new();
            while index < lines.len() - 1 && !lines[index].starts_with("*** ") {
                let raw = lines[index];
                let content = raw
                    .strip_prefix('+')
                    .ok_or_else(|| anyhow!("invalid patch add line: expected `+` prefix"))?;
                add_lines.push(content.to_string());
                index += 1;
            }
            operations.push(PatchOperation::AddFile {
                path,
                lines: add_lines,
            });
            continue;
        }

        if line.starts_with("*** Delete File: ") {
            let path = line
                .trim_start_matches("*** Delete File: ")
                .trim()
                .to_string();
            if path.is_empty() {
                return Err(anyhow!("invalid patch: empty path in Delete File"));
            }
            operations.push(PatchOperation::DeleteFile { path });
            index += 1;
            continue;
        }

        if line.starts_with("*** Move File: ") {
            // Format: *** Move File: from.txt -> to.txt
            let parts = line
                .trim_start_matches("*** Move File: ")
                .trim()
                .split("->")
                .collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(anyhow!(
                    "invalid patch: Move File requires format `from -> to`"
                ));
            }
            let from = parts[0].trim().to_string();
            let to = parts[1].trim().to_string();
            if from.is_empty() || to.is_empty() {
                return Err(anyhow!("invalid patch: empty path in Move File"));
            }
            operations.push(PatchOperation::MoveFile { from, to });
            index += 1;
            continue;
        }

        if line.starts_with("*** Rename File: ") {
            // Format: *** Rename File: old.txt -> new.txt
            let parts = line
                .trim_start_matches("*** Rename File: ")
                .trim()
                .split("->")
                .collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(anyhow!(
                    "invalid patch: Rename File requires format `old -> new`"
                ));
            }
            let from = parts[0].trim().to_string();
            let to = parts[1].trim().to_string();
            if from.is_empty() || to.is_empty() {
                return Err(anyhow!("invalid patch: empty path in Rename File"));
            }
            operations.push(PatchOperation::RenameFile { from, to });
            index += 1;
            continue;
        }

        if line.starts_with("*** Update File: ") {
            let path = line
                .trim_start_matches("*** Update File: ")
                .trim()
                .to_string();
            if path.is_empty() {
                return Err(anyhow!("invalid patch: empty path in Update File"));
            }
            index += 1;
            let mut hunks = Vec::new();
            let mut current_lines = Vec::new();
            while index < lines.len() - 1 && !lines[index].starts_with("*** ") {
                let raw = lines[index];
                if raw.starts_with("@@") {
                    if !current_lines.is_empty() {
                        hunks.push(PatchHunk {
                            lines: std::mem::take(&mut current_lines),
                        });
                    }
                    index += 1;
                    continue;
                }

                let Some((kind, text)) = parse_patch_hunk_line(raw) else {
                    return Err(anyhow!(
                        "invalid patch update line: expected one of ` ` / `+` / `-` / `@@`"
                    ));
                };
                current_lines.push(PatchHunkLine { kind, text });
                index += 1;
            }
            if !current_lines.is_empty() {
                hunks.push(PatchHunk {
                    lines: current_lines,
                });
            }
            if hunks.is_empty() {
                return Err(anyhow!("invalid patch: update file has no hunks"));
            }
            operations.push(PatchOperation::UpdateFile { path, hunks });
            continue;
        }

        return Err(anyhow!("invalid patch: unknown operation line `{line}`"));
    }

    if operations.is_empty() {
        return Err(anyhow!("invalid patch: no operations found"));
    }

    Ok(operations)
}

fn parse_patch_hunk_line(raw: &str) -> Option<(PatchLineKind, String)> {
    if let Some(text) = raw.strip_prefix(' ') {
        return Some((PatchLineKind::Context, text.to_string()));
    }
    if let Some(text) = raw.strip_prefix('-') {
        return Some((PatchLineKind::Remove, text.to_string()));
    }
    if let Some(text) = raw.strip_prefix('+') {
        return Some((PatchLineKind::Add, text.to_string()));
    }
    None
}

async fn plan_patch_actions(
    cwd: &Path,
    operations: &[PatchOperation],
) -> Result<Vec<PlannedAction>> {
    let mut seen_paths = HashSet::new();
    let mut actions = Vec::new();

    for op in operations {
        match op {
            PatchOperation::AddFile { path, lines } => {
                let resolved = resolve_workspace_path_for_write(cwd, path)?;
                if !seen_paths.insert(resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                if resolved.exists() {
                    return Err(anyhow!(
                        "invalid patch: add file target already exists `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                actions.push(PlannedAction::Create {
                    path: resolved,
                    content: lines.join("\n"),
                });
            }
            PatchOperation::DeleteFile { path } => {
                let resolved = resolve_workspace_path(cwd, path)?;
                if !seen_paths.insert(resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                let metadata = tokio::fs::metadata(&resolved).await.with_context(|| {
                    format!("failed to stat delete target: {}", resolved.display())
                })?;
                if !metadata.is_file() {
                    return Err(anyhow!(
                        "invalid patch: delete target is not a file `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                actions.push(PlannedAction::Delete { path: resolved });
            }
            PatchOperation::UpdateFile { path, hunks } => {
                let resolved = resolve_workspace_path(cwd, path)?;
                if !seen_paths.insert(resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                let original = tokio::fs::read_to_string(&resolved)
                    .await
                    .with_context(|| {
                        format!("failed to read update target: {}", resolved.display())
                    })?;
                let updated = apply_hunks_to_content(&original, hunks).with_context(|| {
                    format!(
                        "failed to apply patch to {}",
                        display_rel_path(cwd, &resolved)
                    )
                })?;
                actions.push(PlannedAction::Update {
                    path: resolved,
                    content: updated,
                });
            }
            PatchOperation::MoveFile { from, to } => {
                let from_resolved = resolve_workspace_path(cwd, from)?;
                let to_resolved = resolve_workspace_path_for_write(cwd, to)?;
                if !seen_paths.insert(from_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &from_resolved)
                    ));
                }
                if !seen_paths.insert(to_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                let metadata = tokio::fs::metadata(&from_resolved).await.with_context(|| {
                    format!("failed to stat move source: {}", from_resolved.display())
                })?;
                if !metadata.is_file() {
                    return Err(anyhow!(
                        "invalid patch: move source is not a file `{}`",
                        display_rel_path(cwd, &from_resolved)
                    ));
                }
                if to_resolved.exists() {
                    return Err(anyhow!(
                        "invalid patch: move target already exists `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                actions.push(PlannedAction::Move {
                    from: from_resolved,
                    to: to_resolved,
                });
            }
            PatchOperation::RenameFile { from, to } => {
                // Rename is semantically the same as Move in this implementation
                let from_resolved = resolve_workspace_path(cwd, from)?;
                let to_resolved = resolve_workspace_path_for_write(cwd, to)?;
                if !seen_paths.insert(from_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &from_resolved)
                    ));
                }
                if !seen_paths.insert(to_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                let metadata = tokio::fs::metadata(&from_resolved).await.with_context(|| {
                    format!("failed to stat rename source: {}", from_resolved.display())
                })?;
                if !metadata.is_file() {
                    return Err(anyhow!(
                        "invalid patch: rename source is not a file `{}`",
                        display_rel_path(cwd, &from_resolved)
                    ));
                }
                if to_resolved.exists() {
                    return Err(anyhow!(
                        "invalid patch: rename target already exists `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                actions.push(PlannedAction::Move {
                    from: from_resolved,
                    to: to_resolved,
                });
            }
        }
    }

    Ok(actions)
}

async fn apply_patch_actions(actions: &[PlannedAction]) -> Result<()> {
    let mut backups = HashMap::new();
    for action in actions {
        let path = action.path().to_path_buf();
        let snapshot =
            if tokio::fs::metadata(&path).await.is_ok() {
                Some(tokio::fs::read(&path).await.with_context(|| {
                    format!("failed to read backup snapshot: {}", path.display())
                })?)
            } else {
                None
            };
        backups.insert(path, snapshot);
    }

    let mut applied_paths = Vec::new();
    for action in actions {
        let path = action.path().to_path_buf();
        let apply_result = apply_single_patch_action(action).await;

        if let Err(apply_err) = apply_result {
            rollback_applied_paths(&applied_paths, &backups).await?;
            return Err(apply_err);
        }
        applied_paths.push(path);
    }

    Ok(())
}

async fn apply_single_patch_action(action: &PlannedAction) -> Result<()> {
    match action {
        PlannedAction::Create { path, content } | PlannedAction::Update { path, content } => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("failed to create parent dir: {}", parent.display()))?;
            }
            tokio::fs::write(path, content)
                .await
                .with_context(|| format!("failed to write file: {}", path.display()))?;
        }
        PlannedAction::Delete { path } => {
            tokio::fs::remove_file(path)
                .await
                .with_context(|| format!("failed to delete file: {}", path.display()))?;
        }
        PlannedAction::Move { from, to } => {
            // Ensure parent directory exists
            if let Some(parent) = to.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("failed to create parent dir: {}", parent.display()))?;
            }
            tokio::fs::rename(from, to)
                .await
                .with_context(|| format!("failed to move file: {} -> {}", from.display(), to.display()))?;
        }
    }

    Ok(())
}

async fn rollback_applied_paths(
    applied_paths: &[PathBuf],
    backups: &HashMap<PathBuf, Option<Vec<u8>>>,
) -> Result<()> {
    for path in applied_paths.iter().rev() {
        let Some(snapshot) = backups.get(path) else {
            continue;
        };
        match snapshot {
            Some(bytes) => {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.with_context(|| {
                        format!("rollback failed to create parent dir: {}", parent.display())
                    })?;
                }
                tokio::fs::write(path, bytes).await.with_context(|| {
                    format!("rollback failed to restore file: {}", path.display())
                })?;
            }
            None => {
                if tokio::fs::metadata(path).await.is_ok() {
                    tokio::fs::remove_file(path).await.with_context(|| {
                        format!("rollback failed to remove file: {}", path.display())
                    })?;
                }
            }
        }
    }
    Ok(())
}

fn apply_hunks_to_content(original: &str, hunks: &[PatchHunk]) -> Result<String> {
    let original_lines = split_lines_preserve_end(original);
    let mut new_lines = Vec::new();
    let mut cursor = 0usize;

    for hunk in hunks {
        let expected = hunk
            .lines
            .iter()
            .filter(|line| line.kind != PatchLineKind::Add)
            .map(|line| line.text.clone())
            .collect::<Vec<String>>();

        let match_pos = if expected.is_empty() {
            cursor
        } else {
            find_subsequence(&original_lines, cursor, &expected)
                .ok_or_else(|| anyhow!("hunk context not found"))?
        };

        new_lines.extend_from_slice(&original_lines[cursor..match_pos]);
        let mut source_index = match_pos;

        for line in &hunk.lines {
            match line.kind {
                PatchLineKind::Context => {
                    let source = original_lines.get(source_index).ok_or_else(|| {
                        anyhow!("hunk context out of bounds while applying patch")
                    })?;
                    if source != &line.text {
                        return Err(anyhow!("hunk context mismatch"));
                    }
                    new_lines.push(source.clone());
                    source_index += 1;
                }
                PatchLineKind::Remove => {
                    let source = original_lines.get(source_index).ok_or_else(|| {
                        anyhow!("hunk removal out of bounds while applying patch")
                    })?;
                    if source != &line.text {
                        return Err(anyhow!("hunk removal mismatch"));
                    }
                    source_index += 1;
                }
                PatchLineKind::Add => {
                    new_lines.push(line.text.clone());
                }
            }
        }

        cursor = source_index;
    }

    new_lines.extend_from_slice(&original_lines[cursor..]);
    Ok(new_lines.join("\n"))
}

fn split_lines_preserve_end(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }
    content.split('\n').map(|line| line.to_string()).collect()
}

fn find_subsequence(haystack: &[String], start: usize, needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(start);
    }
    if haystack.len() < needle.len() || start > haystack.len().saturating_sub(needle.len()) {
        return None;
    }
    (start..=haystack.len() - needle.len()).find(|idx| {
        haystack[*idx..*idx + needle.len()]
            .iter()
            .zip(needle)
            .all(|(left, right)| left == right)
    })
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

fn resolve_workspace_path_for_write(cwd: &Path, rel_path: &str) -> Result<PathBuf> {
    let input = Path::new(rel_path);
    if rel_path.trim().is_empty() {
        return Err(anyhow!("path escapes workspace: empty path is not allowed"));
    }
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
    let candidate = cwd.join(input);

    if candidate.exists() {
        let resolved = candidate
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path: {}", candidate.display()))?;
        if !resolved.starts_with(&workspace_root) {
            return Err(anyhow!("path escapes workspace: {}", candidate.display()));
        }
        return Ok(resolved);
    }

    let mut anchor = candidate.as_path();
    while !anchor.exists() {
        anchor = anchor
            .parent()
            .ok_or_else(|| anyhow!("path escapes workspace: {}", candidate.display()))?;
    }

    let anchor_resolved = anchor
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path anchor: {}", anchor.display()))?;
    if !anchor_resolved.starts_with(&workspace_root) {
        return Err(anyhow!("path escapes workspace: {}", candidate.display()));
    }

    Ok(candidate)
}

fn display_rel_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("openjax-tools-test-{pid}-{nanos}-{counter}"))
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

    #[tokio::test]
    async fn apply_patch_add_file_works() {
        let cwd = create_workspace();
        let call = ToolCall {
            name: "apply_patch".to_string(),
            args: HashMap::from([(
                "patch".to_string(),
                "*** Begin Patch\n*** Add File: notes.txt\n+hello\n+world\n*** End Patch"
                    .to_string(),
            )]),
        };

        let output = apply_patch_tool(&call, &cwd)
            .await
            .expect("patch should apply");
        assert!(output.contains("ADD notes.txt"));
        let content = fs::read_to_string(cwd.join("notes.txt")).expect("file should exist");
        assert_eq!(content, "hello\nworld");
        let _ = fs::remove_dir_all(cwd);
    }

    #[tokio::test]
    async fn apply_patch_update_file_works() {
        let cwd = create_workspace();
        fs::write(cwd.join("todo.txt"), "line1\nline2\nline3").expect("seed file");
        let call = ToolCall {
            name: "apply_patch".to_string(),
            args: HashMap::from([(
                "patch".to_string(),
                "*** Begin Patch\n*** Update File: todo.txt\n@@\n line1\n-line2\n+line2-updated\n line3\n*** End Patch"
                    .to_string(),
            )]),
        };

        let output = apply_patch_tool(&call, &cwd)
            .await
            .expect("patch should apply");
        assert!(output.contains("UPDATE "));
        let content = fs::read_to_string(cwd.join("todo.txt")).expect("file should exist");
        assert_eq!(content, "line1\nline2-updated\nline3");
        let _ = fs::remove_dir_all(cwd);
    }

    #[tokio::test]
    async fn apply_patch_rejects_escape_path() {
        let cwd = create_workspace();
        let call = ToolCall {
            name: "apply_patch".to_string(),
            args: HashMap::from([(
                "patch".to_string(),
                "*** Begin Patch\n*** Add File: ../pwned.txt\n+oops\n*** End Patch".to_string(),
            )]),
        };

        let err = apply_patch_tool(&call, &cwd)
            .await
            .expect_err("patch should be rejected");
        assert!(err.to_string().contains("parent traversal is not allowed"));
        assert!(!cwd.join("../pwned.txt").exists());
        let _ = fs::remove_dir_all(cwd);
    }

    #[tokio::test]
    async fn apply_patch_move_file_works() {
        let cwd = create_workspace();
        // Create source file
        fs::write(cwd.join("old.txt"), "hello").expect("seed file");

        let call = ToolCall {
            name: "apply_patch".to_string(),
            args: HashMap::from([(
                "patch".to_string(),
                "*** Begin Patch\n*** Move File: old.txt -> new.txt\n*** End Patch"
                    .to_string(),
            )]),
        };

        let output = apply_patch_tool(&call, &cwd)
            .await
            .expect("patch should apply");
        assert!(output.contains("MOVE"));
        assert!(!cwd.join("old.txt").exists(), "old file should be moved");
        assert!(cwd.join("new.txt").exists(), "new file should exist");
        let content = fs::read_to_string(cwd.join("new.txt")).expect("file should exist");
        assert_eq!(content, "hello");
        let _ = fs::remove_dir_all(cwd);
    }

    #[tokio::test]
    async fn apply_patch_rename_file_works() {
        let cwd = create_workspace();
        // Create source file
        fs::write(cwd.join("file.txt"), "content").expect("seed file");

        let call = ToolCall {
            name: "apply_patch".to_string(),
            args: HashMap::from([(
                "patch".to_string(),
                "*** Begin Patch\n*** Rename File: file.txt -> renamed.txt\n*** End Patch"
                    .to_string(),
            )]),
        };

        let output = apply_patch_tool(&call, &cwd)
            .await
            .expect("patch should apply");
        assert!(output.contains("MOVE"));
        assert!(!cwd.join("file.txt").exists(), "original file should be renamed");
        assert!(cwd.join("renamed.txt").exists(), "renamed file should exist");
        let content = fs::read_to_string(cwd.join("renamed.txt")).expect("file should exist");
        assert_eq!(content, "content");
        let _ = fs::remove_dir_all(cwd);
    }
}
