use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, FunctionCallOutputBody};
use crate::tools::registry::{ToolHandler, ToolKind};
use crate::tools::error::FunctionCallError;

#[derive(Deserialize)]
struct ApplyPatchArgs {
    patch: String,
}

pub struct ApplyPatchHandler;

#[async_trait]
impl ToolHandler for ApplyPatchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "apply_patch handler received unsupported payload".to_string(),
                ));
            }
        };

        debug!(
            raw_arguments = %arguments,
            cwd = %turn.cwd.display(),
            "apply_patch parsing arguments"
        );

        let args: ApplyPatchArgs = serde_json::from_str(&arguments)
            .map_err(|e| {
                debug!(error = %e, raw_arguments = %arguments, "apply_patch failed to parse arguments");
                FunctionCallError::Internal(format!("failed to parse arguments: {}", e))
            })?;

        let patch_arg = args.patch;
        let normalized_patch = normalize_patch_arg(&patch_arg);
        
        debug!(
            patch_len = normalized_patch.len(),
            patch_preview = %normalized_patch.chars().take(200).collect::<String>(),
            "apply_patch normalized patch"
        );

        let operations = crate::tools::parse_apply_patch(&normalized_patch)
            .map_err(|e| FunctionCallError::Internal(format!("failed to apply patch: {}", e)))?;
        
        debug!(
            operation_count = operations.len(),
            operations = ?operations,
            "apply_patch parsed operations"
        );

        let actions = crate::tools::plan_patch_actions(&turn.cwd, &operations).await
            .map_err(|e| FunctionCallError::Internal(format!("failed to apply patch: {}", e)))?;
        crate::tools::apply_patch_actions(&actions).await
            .map_err(|e| FunctionCallError::Internal(format!("failed to apply patch: {}", e)))?;

        let summary = actions
            .iter()
            .map(|action| action.summary(&turn.cwd))
            .collect::<Vec<String>>()
            .join("\n");

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(format!("patch applied successfully\n{summary}")),
            success: Some(true),
        })
    }
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
