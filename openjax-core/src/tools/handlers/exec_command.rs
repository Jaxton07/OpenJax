use async_trait::async_trait;
use crate::tools::context::{ToolInvocation, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

pub struct ExecCommandHandler;

#[async_trait]
impl ToolHandler for ExecCommandHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        Err(FunctionCallError::Internal("exec_command handler not yet implemented".to_string()))
    }
}
