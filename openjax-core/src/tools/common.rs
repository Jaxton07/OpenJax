use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use std::path::{Component, Path, PathBuf};

pub fn parse_tool_args<T: DeserializeOwned>(
    args: &std::collections::HashMap<String, String>,
) -> Result<T> {
    let json_str =
        serde_json::to_string(args).map_err(|e| anyhow!("failed to serialize args: {e}"))?;
    serde_json::from_str(&json_str).map_err(|e| anyhow!("failed to parse arguments: {e}"))
}

pub async fn verify_path_exists(path: &Path) -> Result<()> {
    tokio::fs::metadata(path)
        .await
        .with_context(|| format!("unable to access `{}`", path.display()))?;
    Ok(())
}

pub fn take_bytes_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn resolve_workspace_path(cwd: &Path, rel_path: &str) -> Result<PathBuf> {
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

pub fn resolve_workspace_path_for_write(cwd: &Path, rel_path: &str) -> Result<PathBuf> {
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

pub fn contains_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}
