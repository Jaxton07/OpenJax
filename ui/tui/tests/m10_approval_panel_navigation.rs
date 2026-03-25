use openjax_protocol::Event;
use tui_next::app::{App, SubmitAction};
use tui_next::state::ApprovalSelection;

#[test]
fn approval_panel_supports_up_down_and_submit() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-1".to_string(),
        target: "修改文件 test.txt".to_string(),
        reason: "需要写入文件".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("echo hi > test.txt".to_string()),
        risk_tags: vec!["write".to_string()],
        sandbox_backend: Some("linux_native".to_string()),
        degrade_reason: None,
        policy_version: None,
        matched_rule_id: None,
        approval_kind: None,
    });

    let lines = app.approval_panel_lines().expect("panel should exist");
    assert!(lines.iter().any(|l| l.to_string().contains("Approve")));

    app.move_approval_selection(1);
    let lines = app.approval_panel_lines().expect("panel should exist");
    assert!(lines.iter().any(|l| l.to_string().contains("› Deny")));

    let action = app.submit_input();
    assert!(matches!(
        action,
        Some(SubmitAction::ApprovalDecision {
            request_id,
            approved: false
        }) if request_id == "req-1"
    ));
    let pending = app
        .state
        .pending_approval
        .as_ref()
        .expect("approval should remain pending until resolve completes");
    assert_eq!(pending.request_id, "req-1");
    assert_eq!(app.state.approval_selection, ApprovalSelection::Deny);
    let live_message = app
        .state
        .live_messages
        .first()
        .expect("approval live message should remain visible");
    assert!(
        live_message.content.contains("pending (req-1)"),
        "approval submit should not clear the pending live message early",
    );
}
