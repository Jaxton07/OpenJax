use async_trait::async_trait;
use openjax_core::{ApprovalHandler, ApprovalRequest};
use std::collections::HashMap;
use tokio::sync::{Mutex, oneshot};

#[derive(Debug, Default)]
pub struct TuiApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl TuiApprovalHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn resolve(&self, request_id: &str, approved: bool) -> bool {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(request_id) {
            let _ = tx.send(approved);
            return true;
        }
        false
    }
}

#[async_trait]
impl ApprovalHandler for TuiApprovalHandler {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().await;
        pending.insert(request.request_id, tx);
        drop(pending);

        rx.await
            .map_err(|e| format!("approval decision channel closed: {e}"))
    }
}
