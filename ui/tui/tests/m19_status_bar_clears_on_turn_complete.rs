use openjax_protocol::Event;
use tui_next::app::App;

#[test]
fn turn_completed_clears_status_bar() {
    let mut app = App::default();
    app.apply_core_event(Event::TurnStarted { turn_id: 42 });
    assert!(app.state.status_bar.is_some());

    app.apply_core_event(Event::TurnCompleted { turn_id: 42 });
    assert!(app.state.status_bar.is_none());
}
