use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn assistant_delta_merges_into_single_message() {
    let mut app = App::default();

    app.handle_event(AppEvent::CoreEvent(Event::AssistantDelta {
        turn_id: 1,
        content_delta: "Hel".to_string(),
    }));
    app.handle_event(AppEvent::CoreEvent(Event::AssistantDelta {
        turn_id: 1,
        content_delta: "lo".to_string(),
    }));

    assert_eq!(app.state.messages.len(), 1);
    assert_eq!(app.state.messages[0].role, "assistant");
    assert_eq!(app.state.messages[0].content, "Hello");
}
