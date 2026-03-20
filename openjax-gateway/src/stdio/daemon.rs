//! Daemon session state and approval handler.

use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool};

use async_trait::async_trait;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, approval_timeout_ms_from_env};
use tokio::sync::{Mutex, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{info, warn};

use super::protocol::{ApprovalLogEvent, log_approval_event};

pub struct SessionState {
    pub agent: Arc<Mutex<Agent>>,
    pub streaming_enabled: Arc<AtomicBool>,
    pub approval_handler: Arc<DaemonApprovalHandler>,
}

#[derive(Default)]
pub struct DaemonApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl DaemonApprovalHandler {
    pub async fn resolve(&self, request_id: &str, approved: bool) -> bool {
        let tx = {
            let mut pending = self.pending.lock().await;
            pending.remove(request_id)
        };
        match tx {
            Some(tx) => tx.send(approved).is_ok(),
            None => {
                warn!(approval_request_id = %request_id, "approval request not found");
                log_approval_event(ApprovalLogEvent {
                    action: "resolve_missing",
                    request_id: Some(request_id),
                    turn_id: None,
                    target: None,
                    approved: Some(approved),
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("request_not_found"),
                });
                false
            }
        }
    }
}

#[async_trait]
impl ApprovalHandler for DaemonApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> std::result::Result<bool, String> {
        let request_id = request.request_id.clone();
        let timeout_ms = approval_timeout_ms_from_env();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id.clone(), tx);
        }
        info!(approval_request_id = %request_id, "approval requested");
        log_approval_event(ApprovalLogEvent {
            action: "requested",
            request_id: Some(&request_id),
            turn_id: None,
            target: Some(&request.target),
            approved: None,
            resolved: None,
            session_id: None,
            detail: Some("handler_waiting"),
        });

        let decision = timeout(Duration::from_millis(timeout_ms), rx).await;
        let mut pending = self.pending.lock().await;
        pending.remove(&request_id);

        match decision {
            Ok(Ok(approved)) => {
                log_approval_event(ApprovalLogEvent {
                    action: "handler_decided",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: Some(approved),
                    resolved: Some(true),
                    session_id: None,
                    detail: Some("decision_received"),
                });
                Ok(approved)
            }
            Ok(Err(_)) => {
                warn!(approval_request_id = %request_id, "approval channel closed");
                log_approval_event(ApprovalLogEvent {
                    action: "handler_error",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: None,
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("channel_closed"),
                });
                Err("approval channel closed".to_string())
            }
            Err(_) => {
                warn!(approval_request_id = %request_id, timeout_ms = timeout_ms, "approval timed out");
                log_approval_event(ApprovalLogEvent {
                    action: "handler_timeout",
                    request_id: Some(&request_id),
                    turn_id: None,
                    target: Some(&request.target),
                    approved: None,
                    resolved: Some(false),
                    session_id: None,
                    detail: Some("timeout"),
                });
                Err("approval timed out".to_string())
            }
        }
    }
}
