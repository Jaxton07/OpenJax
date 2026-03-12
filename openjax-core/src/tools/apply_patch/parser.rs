use super::types::{PatchHunk, PatchHunkLine, PatchLineKind, PatchOperation};
use anyhow::{Result, anyhow};

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

            let mut move_to = None;
            if index < lines.len() - 1 && lines[index].starts_with("*** Move to: ") {
                move_to = Some(
                    lines[index]
                        .trim_start_matches("*** Move to: ")
                        .trim()
                        .to_string(),
                );
                if move_to.as_ref().unwrap().is_empty() {
                    return Err(anyhow!("invalid patch: empty path in Move to"));
                }
                index += 1;
            }

            let mut hunks = Vec::new();
            let mut current_lines = Vec::new();
            let mut current_context = None;
            while index < lines.len() - 1 && !lines[index].starts_with("*** ") {
                let raw = lines[index];
                if let Some(stripped) = raw.strip_prefix("@@") {
                    if !current_lines.is_empty() {
                        hunks.push(PatchHunk {
                            context: current_context,
                            lines: std::mem::take(&mut current_lines),
                        });
                        current_context = None;
                    }
                    let context_part = stripped.trim();
                    if !context_part.is_empty() {
                        current_context = Some(context_part.to_string());
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
                    context: current_context,
                    lines: current_lines,
                });
            }
            if hunks.is_empty() {
                return Err(anyhow!("invalid patch: update file has no hunks"));
            }
            operations.push(PatchOperation::UpdateFile {
                path,
                move_to,
                hunks,
            });
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

#[cfg(test)]
mod tests {
    use super::*;

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
            PatchOperation::UpdateFile { path, hunks, .. } => {
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing `*** Begin Patch`")
        );
    }

    #[test]
    fn parse_apply_patch_invalid_missing_end() {
        let patch = r#"*** Begin Patch
*** Add File: new.txt
+Hello world"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing `*** End Patch`")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no operations found")
        );
    }

    #[test]
    fn parse_apply_patch_invalid_unknown_operation() {
        let patch = r#"*** Begin Patch
*** Unknown: test.txt
*** End Patch"#;
        let result = parse_apply_patch(patch);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown operation")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("format `from -> to`")
        );
    }

    #[test]
    fn parse_apply_patch_with_move() {
        let patch = r#"*** Begin Patch
*** Update File: old.txt
*** Move to: new.txt
@@
 old content
-new content
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::UpdateFile {
                path,
                move_to,
                hunks,
            } => {
                assert_eq!(path, "old.txt");
                assert_eq!(move_to.as_deref(), Some("new.txt"));
                assert_eq!(hunks.len(), 1);
            }
            _ => panic!("Expected UpdateFile operation with move_to"),
        }
    }

    #[test]
    fn parse_apply_patch_with_context() {
        let patch = r#"*** Begin Patch
*** Update File: test.txt
@@ fn main
 fn main() {
-    println!("old");
+    println!("new");
 }
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::UpdateFile { path, hunks, .. } => {
                assert_eq!(path, "test.txt");
                assert_eq!(hunks.len(), 1);
                assert_eq!(hunks[0].context.as_deref(), Some("fn main"));
            }
            _ => panic!("Expected UpdateFile operation"),
        }
    }

    #[test]
    fn parse_apply_patch_empty_context() {
        let patch = r#"*** Begin Patch
*** Update File: test.txt
@@
 context
-old
+new
*** End Patch"#;
        let operations = parse_apply_patch(patch).unwrap();
        assert_eq!(operations.len(), 1);
        match &operations[0] {
            PatchOperation::UpdateFile { path, hunks, .. } => {
                assert_eq!(path, "test.txt");
                assert_eq!(hunks.len(), 1);
                assert_eq!(hunks[0].context.as_deref(), None);
            }
            _ => panic!("Expected UpdateFile operation"),
        }
    }
}
