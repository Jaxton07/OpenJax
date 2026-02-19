use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::path::PathBuf;

#[allow(unused_imports)]
use super::types::{PatchHunk, PatchHunkLine, PatchLineKind, PlannedAction};
use super::matcher::{split_lines_preserve_end, find_subsequence};

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

pub fn apply_hunks_to_content(original: &str, hunks: &[PatchHunk]) -> Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
            context: None,
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
            context: None,
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
            context: None,
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
            context: None,
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
}
