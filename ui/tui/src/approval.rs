use async_trait::async_trait;
use openjax_core::{ApprovalHandler, ApprovalRequest};
use std::collections::{HashMap, VecDeque};
use tokio::sync::{Mutex, oneshot};

#[derive(Debug, Default)]
pub struct TuiApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    queued_requests: Mutex<VecDeque<ApprovalRequest>>,
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

    pub async fn pop_request(&self) -> Option<ApprovalRequest> {
        self.queued_requests.lock().await.pop_front()
    }
}

#[async_trait]
impl ApprovalHandler for TuiApprovalHandler {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String> {
        let (tx, rx) = oneshot::channel();
        self.queued_requests.lock().await.push_back(request.clone());
        self.pending.lock().await.insert(request.request_id, tx);
        rx.await
            .map_err(|e| format!("approval decision channel closed: {e}"))
    }
}
