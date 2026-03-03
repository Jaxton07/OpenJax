use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FunctionCallError {
    #[error("{0}")]
    RespondToModel(String),
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Invalid payload for tool {0}")]
    InvalidPayload(String),
    #[error("Approval rejected: {0}")]
    ApprovalRejected(String),
    #[error("Approval timed out: {0}")]
    ApprovalTimedOut(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for FunctionCallError {
    fn from(err: anyhow::Error) -> Self {
        FunctionCallError::Internal(err.to_string())
    }
}
