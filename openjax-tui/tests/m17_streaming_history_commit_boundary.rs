use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn streaming_message_is_not_emitted_until_final_message_arrives() {
    let mut app = App::default();
    app.state.push_user_message("older-1".to_string());
    app.state.push_assistant_message(
        "older-2".to_string(),
        openjax_tui::state::RenderKind::Markdown,
    );
    app.state.push_user_message("older-3".to_string());

    app.handle_event(AppEvent::CoreEvent(Event::TurnStarted { turn_id: 1 }));
    app.handle_event(AppEvent::CoreEvent(Event::AssistantDelta {
        turn_id: 1,
        content_delta: "hel".to_string(),
    }));

    let during_stream = app.collect_new_history_lines_for_inline();
    assert!(
        !during_stream
            .iter()
            .any(|line| line.to_string().contains("hel")),
        "streaming delta text must not be emitted into history"
    );
    assert!(
        !app.live_chat_lines()
            .iter()
            .any(|line| line.to_string().contains("hel")),
        "live viewport should not show in-flight stream text when typewriter is disabled"
    );

    app.handle_event(AppEvent::CoreEvent(Event::AssistantMessage {
        turn_id: 1,
        content: "hello".to_string(),
    }));

    let _after_final = app.collect_new_history_lines_for_inline();
    assert!(
        app.live_chat_lines()
            .iter()
            .any(|line| line.to_string().contains("hello")),
        "after final message, live viewport should show finalized assistant content"
    );
}
