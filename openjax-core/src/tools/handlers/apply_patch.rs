use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

pub struct ApplyPatchHandler;

#[async_trait]
impl ToolHandler for ApplyPatchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        Err(FunctionCallError::Internal("apply_patch handler not yet implemented".to_string()))
    }
}
