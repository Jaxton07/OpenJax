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
    pub fn path(&self) -> &Path {
        match self {
            Self::Create { path, .. }
            | Self::Update { path, .. }
            | Self::Delete { path }
            | Self::Move { to: path, .. } => {
                path.as_path()
            }
        }
    }

    pub fn summary(&self, cwd: &Path) -> String {
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

pub fn normalize_patch_arg(raw: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_apply_patch_add_file() {
        let patch = r#"*** Begin Patch
*** Add File: new.txt
+Hello world
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::AddFile { path, lines } => {
                assert_eq!(path, "new.txt");
                assert_eq!(lines, &["Hello world"]);
            }
            _ => panic!("Expected AddFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_delete_file() {
        let patch = r#"*** Begin Patch
*** Delete File: old.txt
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::DeleteFile { path } => {
                assert_eq!(path, "old.txt");
            }
            _ => panic!("Expected DeleteFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_move_file() {
        let patch = r#"*** Begin Patch
*** Move File: old.txt -> new.txt
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::MoveFile { from, to } => {
                assert_eq!(from, "old.txt");
                assert_eq!(to, "new.txt");
            }
            _ => panic!("Expected MoveFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_rename_file() {
        let patch = r#"*** Begin Patch
*** Rename File: old.txt -> new.txt
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::RenameFile { from, to } => {
                assert_eq!(from, "old.txt");
                assert_eq!(to, "new.txt");
            }
            _ => panic!("Expected RenameFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_update_file() {
        let patch = r#"*** Begin Patch
*** Update File: test.txt
@@
 context line
-old line
+new line
 another context
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::UpdateFile { path, hunks } => {
                assert_eq!(path, "test.txt");
                assert_eq!(hunks.len(), 1);
                assert_eq!(hunks[0].lines.len(), 4);
            }
            _ => panic!("Expected UpdateFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_multiple_operations() {
        let patch = r#"*** Begin Patch
*** Add File: new.txt
+Hello world
*** Update File: test.txt
 context
-old
+new
*** Delete File: old.txt
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 3);
    }

    #[test]
    fn parse_apply_patch_invalid_missing_begin() {
        let patch = r#"*** Add File: new.txt
+Hello world
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing `*** Begin Patch`"));
    }

    #[test]
    fn parse_apply_patch_invalid_missing_end() {
        let patch = r#"*** Begin Patch
*** Add File: new.txt
+Hello world"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing `*** End Patch`"));
    }

    #[test]
    fn parse_apply_patch_invalid_empty_path() {
        let patch = r#"*** Begin Patch
*** Add File: 
+Hello world
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty path"));
    }

    #[test]
    fn parse_apply_patch_invalid_no_operations() {
        let patch = r#"*** Begin Patch
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no operations found"));
    }

    #[test]
    fn parse_apply_patch_invalid_unknown_operation() {
        let patch = r#"*** Begin Patch
*** Unknown: test.txt
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown operation"));
    }

    #[test]
    fn parse_apply_patch_invalid_update_no_hunks() {
        let patch = r#"*** Begin Patch
*** Update File: test.txt
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no hunks"));
    }

    #[test]
    fn parse_apply_patch_invalid_move_format() {
        let patch = r#"*** Begin Patch
*** Move File: old.txt
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("format `from -> to`"));
    }

    #[tokio::test]
    async fn plan_patch_actions_add_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();

        let operations = vec![PatchOperation::AddFile {
            path: "new.txt".to_string(),
            lines: vec!["Hello world".to_string()],
        }];

        let actions = plan_patch_actions(cwd, &operations).await.unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            PlannedAction::Create { path, content } => {
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "new.txt");
                assert_eq!(content, "Hello world");
            }
            _ => panic!("Expected Create action"),
        }
    }

    #[tokio::test]
    async fn plan_patch_actions_delete_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("old.txt");
        tokio::fs::write(&file_path, "content").await.expect("write file");

        let operations = vec![PatchOperation::DeleteFile {
            path: "old.txt".to_string(),
        }];

        let actions = plan_patch_actions(cwd, &operations).await.unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            PlannedAction::Delete { path } => {
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "old.txt");
            }
            _ => panic!("Expected Delete action"),
        }
    }

    #[tokio::test]
    async fn plan_patch_actions_move_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let from_path = cwd.join("old.txt");
        tokio::fs::write(&from_path, "content").await.expect("write file");

        let operations = vec![PatchOperation::MoveFile {
            from: "old.txt".to_string(),
            to: "new.txt".to_string(),
        }];

        let actions = plan_patch_actions(cwd, &operations).await.unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            PlannedAction::Move { from, to } => {
                assert_eq!(from.file_name().unwrap().to_str().unwrap(), "old.txt");
                assert_eq!(to.file_name().unwrap().to_str().unwrap(), "new.txt");
            }
            _ => panic!("Expected Move action"),
        }
    }

    #[tokio::test]
    async fn plan_patch_actions_update_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3\n").await.expect("write file");

        let operations = vec![PatchOperation::UpdateFile {
            path: "test.txt".to_string(),
            hunks: vec![PatchHunk {
                lines: vec![
                    PatchHunkLine { kind: PatchLineKind::Context, text: "line1".to_string() },
                    PatchHunkLine { kind: PatchLineKind::Remove, text: "line2".to_string() },
                    PatchHunkLine { kind: PatchLineKind::Add, text: "line2-updated".to_string() },
                    PatchHunkLine { kind: PatchLineKind::Context, text: "line3".to_string() },
                ],
            }],
        }];

        let actions = plan_patch_actions(cwd, &operations).await.unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            PlannedAction::Update { path, content } => {
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "test.txt");
                assert_eq!(content, "line1\nline2-updated\nline3\n");
            }
            _ => panic!("Expected Update action"),
        }
    }

    #[tokio::test]
    async fn plan_patch_actions_duplicate_target() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();

        let operations = vec![
            PatchOperation::AddFile {
                path: "test.txt".to_string(),
                lines: vec!["content".to_string()],
            },
            PatchOperation::UpdateFile {
                path: "test.txt".to_string(),
                hunks: vec![],
            },
        ];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("duplicated file operation target"));
    }

    #[tokio::test]
    async fn plan_patch_actions_add_existing_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("test.txt");
        tokio::fs::write(&file_path, "content").await.expect("write file");

        let operations = vec![PatchOperation::AddFile {
            path: "test.txt".to_string(),
            lines: vec!["new content".to_string()],
        }];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("add file target already exists"));
    }

    #[tokio::test]
    async fn plan_patch_actions_update_nonexistent_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();

        let operations = vec![PatchOperation::UpdateFile {
            path: "test.txt".to_string(),
            hunks: vec![],
        }];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("update file target does not exist"));
    }

    #[tokio::test]
    async fn apply_patch_actions_creates_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();

        let actions = vec![PlannedAction::Create {
            path: cwd.join("new.txt"),
            content: "Hello world".to_string(),
        }];

        apply_patch_actions(&actions).await.unwrap();
        let content = tokio::fs::read_to_string(cwd.join("new.txt")).await.unwrap();
        assert_eq!(content, "Hello world");
    }

    #[tokio::test]
    async fn apply_patch_actions_deletes_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("old.txt");
        tokio::fs::write(&file_path, "content").await.expect("write file");

        let actions = vec![PlannedAction::Delete {
            path: file_path.clone(),
        }];

        apply_patch_actions(&actions).await.unwrap();
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn apply_patch_actions_updates_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("test.txt");
        tokio::fs::write(&file_path, "old content").await.expect("write file");

        let actions = vec![PlannedAction::Update {
            path: file_path.clone(),
            content: "new content".to_string(),
        }];

        apply_patch_actions(&actions).await.unwrap();
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn apply_patch_actions_moves_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let from_path = cwd.join("old.txt");
        let to_path = cwd.join("new.txt");
        tokio::fs::write(&from_path, "content").await.expect("write file");

        let actions = vec![PlannedAction::Move {
            from: from_path.clone(),
            to: to_path.clone(),
        }];

        apply_patch_actions(&actions).await.unwrap();
        assert!(!from_path.exists());
        assert!(to_path.exists());
        let content = tokio::fs::read_to_string(&to_path).await.unwrap();
        assert_eq!(content, "content");
    }

    #[tokio::test]
    async fn apply_patch_actions_rolls_back_on_error() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file1_path = cwd.join("file1.txt");
        let file2_path = cwd.join("file2.txt");
        tokio::fs::write(&file1_path, "content1").await.expect("write file1");
        tokio::fs::write(&file2_path, "content2").await.expect("write file2");

        let actions = vec![
            PlannedAction::Update {
                path: file1_path.clone(),
                content: "new content1".to_string(),
            },
            PlannedAction::Update {
                path: PathBuf::from("/nonexistent/path.txt"),
                content: "new content2".to_string(),
            },
        ];

        let result = apply_patch_actions(&actions).await;
        assert!(result.is_err());

        let content1 = tokio::fs::read_to_string(&file1_path).await.unwrap();
        assert_eq!(content1, "content1");

        let content2 = tokio::fs::read_to_string(&file2_path).await.unwrap();
        assert_eq!(content2, "content2");
    }

    #[test]
    fn apply_hunks_to_content_simple_replace() {
        let original = "line1\nline2\nline3\n";
        let hunks = vec![PatchHunk {
            lines: vec![
                PatchHunkLine { kind: PatchLineKind::Context, text: "line1".to_string() },
                PatchHunkLine { kind: PatchLineKind::Remove, text: "line2".to_string() },
                PatchHunkLine { kind: PatchLineKind::Add, text: "line2-new".to_string() },
                PatchHunkLine { kind: PatchLineKind::Context, text: "line3".to_string() },
            ],
        }];

        let result = apply_hunks_to_content(original, &hunks).unwrap();
        assert_eq!(result, "line1\nline2-new\nline3\n");
    }

    #[test]
    fn apply_hunks_to_content_multiple_removals() {
        let original = "line1\nline2\nline3\nline4\n";
        let hunks = vec![PatchHunk {
            lines: vec![
                PatchHunkLine { kind: PatchLineKind::Context, text: "line1".to_string() },
                PatchHunkLine { kind: PatchLineKind::Remove, text: "line2".to_string() },
                PatchHunkLine { kind: PatchLineKind::Remove, text: "line3".to_string() },
                PatchHunkLine { kind: PatchLineKind::Context, text: "line4".to_string() },
            ],
        }];

        let result = apply_hunks_to_content(original, &hunks).unwrap();
        assert_eq!(result, "line1\nline4\n");
    }

    #[test]
    fn apply_hunks_to_content_multiple_additions() {
        let original = "line1\nline2\n";
        let hunks = vec![PatchHunk {
            lines: vec![
                PatchHunkLine { kind: PatchLineKind::Context, text: "line1".to_string() },
                PatchHunkLine { kind: PatchLineKind::Add, text: "line1.5".to_string() },
                PatchHunkLine { kind: PatchLineKind::Context, text: "line2".to_string() },
            ],
        }];

        let result = apply_hunks_to_content(original, &hunks).unwrap();
        assert_eq!(result, "line1\nline1.5\nline2\n");
    }

    #[test]
    fn apply_hunks_to_content_context_mismatch() {
        let original = "line1\nline2\nline3\n";
        let hunks = vec![PatchHunk {
            lines: vec![
                PatchHunkLine { kind: PatchLineKind::Context, text: "line1".to_string() },
                PatchHunkLine { kind: PatchLineKind::Remove, text: "line2".to_string() },
                PatchHunkLine { kind: PatchLineKind::Context, text: "line3-different".to_string() },
            ],
        }];

        let result = apply_hunks_to_content(original, &hunks);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("hunk context not found"));
    }

    #[test]
    fn find_subsequence_empty_needle() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = find_subsequence(&haystack, 0, &[]);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn find_subsequence_found() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["b".to_string(), "c".to_string()];
        let result = find_subsequence(&haystack, 0, &needle);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn find_subsequence_not_found() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["d".to_string()];
        let result = find_subsequence(&haystack, 0, &needle);
        assert_eq!(result, None);
    }

    #[test]
    fn find_subsequence_start_offset() {
        let haystack = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let needle = vec!["b".to_string()];
        let result = find_subsequence(&haystack, 1, &needle);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn display_rel_path_relative() {
        let cwd = Path::new("/workspace");
        let path = Path::new("/workspace/src/file.txt");
        let result = display_rel_path(cwd, &path);
        assert_eq!(result, "src/file.txt");
    }

    #[test]
    fn display_rel_path_absolute() {
        let cwd = Path::new("/workspace");
        let path = Path::new("/other/file.txt");
        let result = display_rel_path(cwd, &path);
        assert_eq!(result, "/other/file.txt");
    }
}
