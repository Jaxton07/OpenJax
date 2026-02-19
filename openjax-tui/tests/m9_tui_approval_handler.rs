use openjax_core::{ApprovalHandler, ApprovalRequest};
use openjax_tui::approval::TuiApprovalHandler;
use std::sync::Arc;

#[tokio::test]
async fn approval_handler_waits_and_resolves_from_ui_decision() {
    let handler = Arc::new(TuiApprovalHandler::new());
    let waiter_handler = Arc::clone(&handler);
    let wait_task = tokio::spawn(async move {
        waiter_handler
            .request_approval(ApprovalRequest {
                request_id: "req-123".to_string(),
                target: "shell".to_string(),
                reason: "needs approval".to_string(),
            })
            .await
    });

    let mut resolved = false;
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        if handler.resolve("req-123", true).await {
            resolved = true;
            break;
        }
    }
    assert!(resolved);

    let decision = wait_task
        .await
        .expect("wait task should complete")
        .expect("approval should resolve");
    assert!(decision);
}
