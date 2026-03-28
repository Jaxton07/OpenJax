use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

#[derive(Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

pub struct WriteFileHandler;

#[async_trait]
impl ToolHandler for WriteFileHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "write_file handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: WriteFileArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
        })?;

        let path = crate::tools::resolve_workspace_path_for_write(&turn.cwd, &args.file_path)
            .map_err(|e| FunctionCallError::Internal(e.to_string()))?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                FunctionCallError::Internal(format!("failed to create parent directories: {}", e))
            })?;
        }

        tokio::fs::write(&path, &args.content)
            .await
            .map_err(|e| FunctionCallError::Internal(format!("failed to write file: {}", e)))?;

        let response = format!(
            "written {} ({} bytes)",
            Path::new(&args.file_path).display(),
            args.content.len()
        );

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(response),
            success: Some(true),
        })
    }
}
