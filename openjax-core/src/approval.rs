use async_trait::async_trait;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub target: String,
    pub reason: String,
}

#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String>;
}

#[derive(Debug, Default)]
pub struct StdinApprovalHandler;

impl StdinApprovalHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ApprovalHandler for StdinApprovalHandler {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String> {
        println!("[approval] 执行需要确认: {}", request.target);
        println!("[approval] request id: {}", request.request_id);
        println!("[approval] 原因: {}", request.reason);
        println!("[approval] 输入 y 同意，其他任意输入拒绝:");

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|e| format!("failed to read approval input: {e}"))?;

        Ok(answer.trim().eq_ignore_ascii_case("y"))
    }
}
