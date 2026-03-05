use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn turn_started_uses_status_bar_instead_of_live_status_message() {
    let mut app = App::default();
    app.apply_core_event(Event::TurnStarted { turn_id: 1 });

    let has_live_status = app
        .state
        .live_messages
        .iter()
        .any(|message| message.role == "status");
    assert!(!has_live_status);
    assert!(app.state.status_bar.is_some());
}
