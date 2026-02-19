use async_trait::async_trait;
use serde::de::{self, Deserializer};
use serde::Deserialize;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

#[derive(Deserialize)]
struct EditFileRangeArgs {
    #[serde(alias = "path", alias = "filepath")]
    file_path: String,
    #[serde(deserialize_with = "de_usize")]
    start_line: usize,
    #[serde(deserialize_with = "de_usize")]
    end_line: usize,
    new_text: String,
}

fn de_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrString {
        Num(usize),
        Str(String),
    }

    match NumOrString::deserialize(deserializer)? {
        NumOrString::Num(n) => Ok(n),
        NumOrString::Str(s) => s
            .trim()
            .parse::<usize>()
            .map_err(|_| de::Error::custom("expected positive integer")),
    }
}

pub struct EditFileRangeHandler;

#[async_trait]
impl ToolHandler for EditFileRangeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "edit_file_range handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: EditFileRangeArgs = serde_json::from_str(&arguments)
            .map_err(|e| FunctionCallError::Internal(format!("failed to parse arguments: {}", e)))?;

        if args.start_line == 0 || args.end_line == 0 {
            return Err(FunctionCallError::RespondToModel(
                "start_line and end_line must be 1-indexed line numbers".to_string(),
            ));
        }
        if args.start_line > args.end_line {
            return Err(FunctionCallError::RespondToModel(
                "start_line must be less than or equal to end_line".to_string(),
            ));
        }

        let path = crate::tools::resolve_workspace_path_for_write(&turn.cwd, &args.file_path)
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;

        let original = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| FunctionCallError::Internal(format!("failed to read file: {}", e)))?;

        let had_trailing_newline = original.ends_with('\n');
        let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
        if lines.is_empty() {
            return Err(FunctionCallError::RespondToModel("file is empty".to_string()));
        }

        let line_count = lines.len();
        if args.end_line > line_count {
            return Err(FunctionCallError::RespondToModel(format!(
                "end_line {} exceeds file length {}",
                args.end_line, line_count
            )));
        }

        let replacement: Vec<String> = if args.new_text.is_empty() {
            Vec::new()
        } else {
            args.new_text.lines().map(|l| l.to_string()).collect()
        };

        let start_idx = args.start_line - 1;
        let end_idx_exclusive = args.end_line;
        lines.splice(start_idx..end_idx_exclusive, replacement);

        let mut updated = lines.join("\n");
        if had_trailing_newline {
            updated.push('\n');
        }

        tokio::fs::write(&path, updated)
            .await
            .map_err(|e| FunctionCallError::Internal(format!("failed to write file: {}", e)))?;

        let summary = format!(
            "edit applied successfully\nUPDATE {}:{}-{}",
            args.file_path, args.start_line, args.end_line
        );

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(summary),
            success: Some(true),
        })
    }
}
