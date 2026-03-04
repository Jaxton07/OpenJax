use openjax_protocol::Event;
use tui_next::app::{App, SubmitAction};

#[test]
fn approval_history_is_written_only_on_resolved_event() {
    let mut app = App::default();
    app.apply_core_event(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-123".to_string(),
        target: "git commit -m \"x\"".to_string(),
        reason: "approval required".to_string(),
        tool_name: Some("shell".to_string()),
        command_preview: Some("git commit -m \"x\"".to_string()),
        risk_tags: vec!["write".to_string()],
        sandbox_backend: Some("macos_seatbelt".to_string()),
        degrade_reason: None,
    });

    app.state.input = "y".to_string();
    let action = app.submit_input();
    assert!(matches!(
        action,
        Some(SubmitAction::ApprovalDecision {
            request_id,
            approved: true
        }) if request_id == "req-123"
    ));
    assert!(
        app.drain_history_cells().is_empty(),
        "local approval input should not create duplicate history rows"
    );

    app.apply_core_event(Event::ApprovalResolved {
        turn_id: 1,
        request_id: "req-123".to_string(),
        approved: true,
    });
    let cells = app.drain_history_cells();
    assert_eq!(cells.len(), 1);
    let text = cells[0]
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("approval resolved approved (req-123)"));
}
