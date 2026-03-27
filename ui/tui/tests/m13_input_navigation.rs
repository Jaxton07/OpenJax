use tui_next::app::App;
use tui_next::state::{ApprovalSelection, PendingApproval};

#[test]
fn input_cursor_supports_left_right_and_mid_insert() {
    let mut app = App::default();
    app.append_input("abcd");
    app.move_cursor_left();
    app.move_cursor_left();
    app.append_input("X");
    assert_eq!(app.state.input, "abXcd");
    app.backspace();
    assert_eq!(app.state.input, "abcd");
}

#[test]
fn up_down_navigates_input_history_when_no_pending_approval() {
    let mut app = App::default();
    let _ = app.drain_history_cells();

    app.append_input("first");
    let _ = app.submit_input();
    let _ = app.drain_history_cells();

    app.append_input("second");
    let _ = app.submit_input();
    let _ = app.drain_history_cells();

    app.history_prev();
    assert_eq!(app.state.input, "second");
    app.history_prev();
    assert_eq!(app.state.input, "first");
    app.history_next();
    assert_eq!(app.state.input, "second");
    app.history_next();
    assert_eq!(app.state.input, "");
}

#[test]
fn up_down_keeps_approval_selection_behavior_when_pending() {
    let mut app = App::default();
    app.state.pending_approval = Some(PendingApproval {
        request_id: "rid".to_string(),
        target: "target".to_string(),
        reason: "reason".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("echo hi".to_string()),
        risk_tags: vec![],
        sandbox_backend: None,
        degrade_reason: None,
    });
    assert_eq!(app.state.approval_selection, ApprovalSelection::Approve);
    app.move_approval_selection(1);
    assert_eq!(app.state.approval_selection, ApprovalSelection::Deny);
}
