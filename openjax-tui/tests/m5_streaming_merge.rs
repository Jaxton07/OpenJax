use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn assistant_delta_does_not_render_until_final_message() {
    let mut app = App::default();

    app.handle_event(AppEvent::CoreEvent(Event::TurnStarted { turn_id: 1 }));
    app.handle_event(AppEvent::CoreEvent(Event::AssistantDelta {
        turn_id: 1,
        content_delta: "Hel".to_string(),
    }));
    app.handle_event(AppEvent::CoreEvent(Event::AssistantDelta {
        turn_id: 1,
        content_delta: "lo".to_string(),
    }));

    assert!(
        app.state.transcript.messages.is_empty(),
        "delta should not render assistant message in transcript"
    );

    app.handle_event(AppEvent::CoreEvent(Event::AssistantMessage {
        turn_id: 1,
        content: "Hello".to_string(),
    }));
    assert_eq!(app.state.transcript.messages.len(), 1);
    assert_eq!(app.state.transcript.messages[0].role, "assistant");
    assert_eq!(app.state.transcript.messages[0].content, "Hello");
}
