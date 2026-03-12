use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use std::path::Path;

use super::applier::apply_hunks_to_content;
use super::matcher::display_rel_path;
use super::types::{PatchOperation, PlannedAction};

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
            PatchOperation::UpdateFile {
                path,
                move_to,
                hunks,
            } => {
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
                let original = tokio::fs::read_to_string(&resolved)
                    .await
                    .with_context(|| format!("failed to read file: {}", resolved.display()))?;
                let content = apply_hunks_to_content(&original, hunks)?;

                if let Some(move_to_path) = move_to {
                    let move_to_resolved =
                        crate::tools::resolve_workspace_path_for_write(cwd, move_to_path)?;
                    if !seen_paths.insert(move_to_resolved.clone()) {
                        return Err(anyhow!(
                            "invalid patch: duplicated file operation target `{}`",
                            display_rel_path(cwd, &move_to_resolved)
                        ));
                    }
                    actions.push(PlannedAction::Move {
                        from: resolved,
                        to: move_to_resolved,
                    });
                    actions.push(PlannedAction::Update {
                        path: crate::tools::resolve_workspace_path_for_write(cwd, move_to_path)?,
                        content,
                    });
                } else {
                    actions.push(PlannedAction::Update {
                        path: resolved,
                        content,
                    });
                }
            }
        }
    }

    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::super::types::{PatchHunk, PatchHunkLine, PatchLineKind};
    use super::*;
    use tempfile::TempDir;

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
        tokio::fs::write(&file_path, "content")
            .await
            .expect("write file");

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
        tokio::fs::write(&from_path, "content")
            .await
            .expect("write file");

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
        tokio::fs::write(&file_path, "line1\nline2\nline3\n")
            .await
            .expect("write file");

        let operations = vec![PatchOperation::UpdateFile {
            path: "test.txt".to_string(),
            move_to: None,
            hunks: vec![PatchHunk {
                context: None,
                lines: vec![
                    PatchHunkLine {
                        kind: PatchLineKind::Context,
                        text: "line1".to_string(),
                    },
                    PatchHunkLine {
                        kind: PatchLineKind::Remove,
                        text: "line2".to_string(),
                    },
                    PatchHunkLine {
                        kind: PatchLineKind::Add,
                        text: "line2-updated".to_string(),
                    },
                    PatchHunkLine {
                        kind: PatchLineKind::Context,
                        text: "line3".to_string(),
                    },
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
                move_to: None,
                hunks: vec![],
            },
        ];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("duplicated file operation target")
        );
    }

    #[tokio::test]
    async fn plan_patch_actions_add_existing_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();
        let file_path = cwd.join("test.txt");
        tokio::fs::write(&file_path, "content")
            .await
            .expect("write file");

        let operations = vec![PatchOperation::AddFile {
            path: "test.txt".to_string(),
            lines: vec!["new content".to_string()],
        }];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("add file target already exists")
        );
    }

    #[tokio::test]
    async fn plan_patch_actions_update_nonexistent_file() {
        let temp = TempDir::new().expect("create tempdir");
        let cwd = temp.path();

        let operations = vec![PatchOperation::UpdateFile {
            path: "test.txt".to_string(),
            move_to: None,
            hunks: vec![],
        }];

        let result = plan_patch_actions(cwd, &operations).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("update file target does not exist")
        );
    }
}
