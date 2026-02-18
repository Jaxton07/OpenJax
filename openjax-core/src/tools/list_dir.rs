use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::common::{parse_tool_args, take_bytes_at_char_boundary};
use crate::tools::ToolCall;

const LIST_DIR_MAX_ENTRY_LENGTH: usize = 500;
const LIST_DIR_INDENTATION_SPACES: usize = 2;

#[derive(Deserialize)]
struct ListDirArgs {
    dir_path: String,
    #[serde(default = "list_dir_default_offset")]
    offset: usize,
    #[serde(default = "list_dir_default_limit")]
    limit: usize,
    #[serde(default = "list_dir_default_depth")]
    depth: usize,
}

#[derive(Clone)]
struct DirEntry {
    name: String,
    display_name: String,
    depth: usize,
    kind: DirEntryKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<&FileType> for DirEntryKind {
    fn from(file_type: &FileType) -> Self {
        if file_type.is_symlink() {
            DirEntryKind::Symlink
        } else if file_type.is_dir() {
            DirEntryKind::Directory
        } else if file_type.is_file() {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        }
    }
}

fn list_dir_default_offset() -> usize { 1 }
fn list_dir_default_limit() -> usize { 25 }
fn list_dir_default_depth() -> usize { 2 }

pub async fn list_dir(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: ListDirArgs = parse_tool_args(&call.args)?;

    if args.offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed entry number"));
    }

    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    if args.depth == 0 {
        return Err(anyhow!("depth must be greater than zero"));
    }

    let rel_path = args.dir_path;
    let path = crate::tools::resolve_workspace_path(cwd, &rel_path)?;

    let entries = list_dir_slice(&path, args.offset, args.limit, args.depth).await?;
    let mut output = Vec::with_capacity(entries.len() + 1);
    output.push(format!("Absolute path: {}", path.display()));
    output.extend(entries);

    Ok(output.join("\n"))
}

async fn list_dir_slice(
    path: &Path,
    offset: usize,
    limit: usize,
    depth: usize,
) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    collect_dir_entries(path, Path::new(""), depth, &mut entries).await?;

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let start_index = offset - 1;
    if start_index >= entries.len() {
        return Err(anyhow!("offset exceeds directory entry count"));
    }

    let remaining_entries = entries.len() - start_index;
    let capped_limit = limit.min(remaining_entries);
    let end_index = start_index + capped_limit;
    let selected_entries = &entries[start_index..end_index];
    let mut formatted = Vec::with_capacity(selected_entries.len());

    for entry in selected_entries {
        formatted.push(format_dir_entry_line(entry));
    }

    if end_index < entries.len() {
        formatted.push(format!("More than {capped_limit} entries found"));
    }

    Ok(formatted)
}

async fn collect_dir_entries(
    dir_path: &Path,
    relative_prefix: &Path,
    depth: usize,
    entries: &mut Vec<DirEntry>,
) -> Result<()> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.to_path_buf(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut read_dir = fs::read_dir(&current_dir).await
            .with_context(|| "failed to read directory")?;

        let mut dir_entries = Vec::new();

        while let Some(entry) = read_dir.next_entry().await
            .with_context(|| "failed to read directory")? {
            let file_type = entry.file_type().await
                .with_context(|| "failed to inspect entry")?;

            let file_name = entry.file_name();
            let relative_path = if prefix.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                prefix.join(&file_name)
            };

            let display_name = format_dir_entry_component(&file_name);
            let display_depth = prefix.components().count();
            let sort_key = format_dir_entry_name(&relative_path);
            let kind = DirEntryKind::from(&file_type);
            dir_entries.push((
                entry.path(),
                relative_path,
                kind,
                DirEntry {
                    name: sort_key,
                    display_name,
                    depth: display_depth,
                    kind,
                },
            ));
        }

        dir_entries.sort_unstable_by(|a, b| a.3.name.cmp(&b.3.name));

        for (entry_path, relative_path, kind, dir_entry) in dir_entries {
            if kind == DirEntryKind::Directory && remaining_depth > 1 {
                queue.push_back((entry_path, relative_path, remaining_depth - 1));
            }
            entries.push(dir_entry);
        }
    }

    Ok(())
}

fn format_dir_entry_name(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace("\\", "/");
    if normalized.len() > LIST_DIR_MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, LIST_DIR_MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized
    }
}

fn format_dir_entry_component(name: &OsStr) -> String {
    let normalized = name.to_string_lossy();
    if normalized.len() > LIST_DIR_MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, LIST_DIR_MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized.to_string()
    }
}

fn format_dir_entry_line(entry: &DirEntry) -> String {
    let indent = " ".repeat(entry.depth * LIST_DIR_INDENTATION_SPACES);
    let mut name = entry.display_name.clone();
    match entry.kind {
        DirEntryKind::Directory => name.push('/'),
        DirEntryKind::Symlink => name.push('@'),
        DirEntryKind::Other => name.push('?'),
        DirEntryKind::File => {}
    }
    format!("{indent}{name}")
}
