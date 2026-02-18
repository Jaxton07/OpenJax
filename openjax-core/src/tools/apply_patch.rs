use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::tools::ToolCall;

#[derive(Debug, Clone)]
pub enum PatchOperation {
    AddFile { path: String, lines: Vec<String> },
    DeleteFile { path: String },
    UpdateFile { path: String, hunks: Vec<PatchHunk> },
    MoveFile { from: String, to: String },
    RenameFile { from: String, to: String },
}

#[derive(Debug, Clone)]
pub struct PatchHunk {
    lines: Vec<PatchHunkLine>,
}

#[derive(Debug, Clone)]
pub struct PatchHunkLine {
    kind: PatchLineKind,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLineKind {
    Context,
    Remove,
    Add,
}

#[derive(Debug, Clone)]
pub enum PlannedAction {
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

pub async fn apply_patch_tool(call: &ToolCall, cwd: &Path) -> Result<String> {
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

pub fn parse_apply_patch(patch: &str) -> Result<Vec<PatchOperation>> {
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

pub async fn plan_patch_actions(
    cwd: &Path,
    operations: &[PatchOperation],
) -> Result<Vec<PlannedAction>> {
    let mut seen_paths = HashSet::new();
    let mut actions = Vec::new();

    for op in operations {
        match op {
            PatchOperation::AddFile { path, lines } => {
                let resolved = crate::tools::resolve_workspace_path_for_write(cwd, path)?;
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
                let resolved = crate::tools::resolve_workspace_path_for_write(cwd, path)?;
                if !seen_paths.insert(resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                actions.push(PlannedAction::Delete { path: resolved });
            }
            PatchOperation::MoveFile { from, to } => {
                let from_resolved = crate::tools::resolve_workspace_path_for_write(cwd, from)?;
                let to_resolved = crate::tools::resolve_workspace_path_for_write(cwd, to)?;
                if !seen_paths.insert(to_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                actions.push(PlannedAction::Move {
                    from: from_resolved,
                    to: to_resolved,
                });
            }
            PatchOperation::RenameFile { from, to } => {
                let from_resolved = crate::tools::resolve_workspace_path_for_write(cwd, from)?;
                let to_resolved = crate::tools::resolve_workspace_path_for_write(cwd, to)?;
                if !seen_paths.insert(to_resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &to_resolved)
                    ));
                }
                actions.push(PlannedAction::Move {
                    from: from_resolved,
                    to: to_resolved,
                });
            }
            PatchOperation::UpdateFile { path, hunks } => {
                let resolved = crate::tools::resolve_workspace_path_for_write(cwd, path)?;
                if !seen_paths.insert(resolved.clone()) {
                    return Err(anyhow!(
                        "invalid patch: duplicated file operation target `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                if !resolved.exists() {
                    return Err(anyhow!(
                        "invalid patch: update file target does not exist `{}`",
                        display_rel_path(cwd, &resolved)
                    ));
                }
                let original = tokio::fs::read_to_string(&resolved).await
                    .with_context(|| format!("failed to read file: {}", resolved.display()))?;
                let content = apply_hunks_to_content(&original, hunks)?;
                actions.push(PlannedAction::Update {
                    path: resolved,
                    content,
                });
            }
        }
    }

    Ok(actions)
}

pub async fn apply_patch_actions(actions: &[PlannedAction]) -> Result<()> {
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

fn display_rel_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}
