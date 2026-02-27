use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn approval_overlay_opens_and_closes_with_events() {
    let mut app = App::default();

    app.handle_event(AppEvent::CoreEvent(Event::ApprovalRequested {
        turn_id: 1,
        request_id: "req-1".to_string(),
        target: "shell".to_string(),
        reason: "needs approval".to_string(),
    }));
    assert!(app.state.approval.overlay.is_some());

    app.handle_event(AppEvent::CoreEvent(Event::ApprovalResolved {
        turn_id: 1,
        request_id: "req-1".to_string(),
        approved: true,
    }));
    assert_eq!(app.state.approval.pending_count(), 0);
    assert!(!app.state.approval.overlay_visible);
}
