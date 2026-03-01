use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn approval_panel_supports_up_down_and_submit() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-1".to_string(),
        target: "修改文件 test.txt".to_string(),
        reason: "需要写入文件".to_string(),
    });

    let lines = app.approval_panel_lines().expect("panel should exist");
    assert!(lines.iter().any(|l| l.to_string().contains("Approve")));

    app.move_approval_selection(1);
    let lines = app.approval_panel_lines().expect("panel should exist");
    assert!(lines.iter().any(|l| l.to_string().contains("› Deny")));

    let action = app.submit_input();
    assert!(action.is_some());
    assert!(
        app.state.live_messages.is_empty(),
        "approval submit should clear transient approval live message",
    );
}
