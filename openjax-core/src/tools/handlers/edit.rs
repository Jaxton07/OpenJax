use async_trait::async_trait;
use serde::Deserialize;
use std::io::ErrorKind;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

#[derive(Deserialize)]
struct EditArgs {
    #[serde(alias = "path", alias = "filepath")]
    file_path: String,
    old_string: String,
    #[serde(alias = "new_text")]
    new_string: String,
}

pub struct EditHandler;

#[async_trait]
impl ToolHandler for EditHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Edit handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: EditArgs = match serde_json::from_str(&arguments) {
            Ok(parsed) => parsed,
            Err(_) => {
                return Ok(failed_result(
                    "invalid_args",
                    "failed to parse arguments for Edit. Call Read before retrying.",
                ));
            }
        };

        if args.file_path.trim().is_empty() {
            return Ok(failed_result(
                "invalid_args",
                "file_path must be non-empty. Call Read before retrying.",
            ));
        }
        if args.old_string.is_empty() {
            return Ok(failed_result(
                "invalid_args",
                "old_string must be non-empty. Call Read before retrying.",
            ));
        }

        let path = crate::tools::resolve_workspace_path_for_write(&turn.cwd, &args.file_path)
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;

        let original = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(failed_result(
                    "file_missing",
                    format!(
                        "file was not found: {}. Call Read before retrying.",
                        args.file_path
                    ),
                ));
            }
            Err(err) => {
                return Err(FunctionCallError::Internal(format!(
                    "failed to read file: {err}"
                )));
            }
        };

        let source_uses_crlf = original.contains("\r\n");
        let normalized_file = normalize_newlines(&original);
        let normalized_old = normalize_newlines(&args.old_string);
        let normalized_new = normalize_newlines(&args.new_string);
        let ranges = find_match_ranges(&normalized_file, &normalized_old);

        if ranges.is_empty() {
            return Ok(failed_result(
                "not_found",
                format!(
                    "old_string was not found in {}. Call Read before retrying.",
                    args.file_path
                ),
            ));
        }
        if ranges.len() > 1 {
            return Ok(failed_result(
                "not_unique",
                format!(
                    "old_string matched multiple locations in {}. Call Read and provide a more specific old_string.",
                    args.file_path
                ),
            ));
        }

        let (start, end) = ranges[0];
        let mut updated_normalized = String::with_capacity(
            normalized_file.len() - normalized_old.len() + normalized_new.len(),
        );
        updated_normalized.push_str(&normalized_file[..start]);
        updated_normalized.push_str(&normalized_new);
        updated_normalized.push_str(&normalized_file[end..]);

        let final_output = if source_uses_crlf {
            updated_normalized.replace('\n', "\r\n")
        } else {
            updated_normalized
        };

        tokio::fs::write(&path, final_output)
            .await
            .map_err(|e| FunctionCallError::Internal(format!("failed to write file: {}", e)))?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(format!(
                "The file {} has been updated successfully.",
                args.file_path
            )),
            success: Some(true),
        })
    }
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn find_match_ranges(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut search_start = 0;

    while search_start <= haystack.len() {
        let remaining = &haystack[search_start..];
        let Some(relative_start) = remaining.find(needle) else {
            break;
        };
        let start = search_start + relative_start;
        let end = start + needle.len();
        ranges.push((start, end));

        let advance = haystack[start..]
            .chars()
            .next()
            .map(|ch| ch.len_utf8())
            .unwrap_or(1);
        search_start = start + advance;
    }

    ranges
}

fn failed_result(class: &str, details: impl AsRef<str>) -> ToolOutput {
    ToolOutput::Function {
        body: FunctionCallOutputBody::Text(format!("Edit failed [{class}]: {}", details.as_ref())),
        success: Some(false),
    }
}
