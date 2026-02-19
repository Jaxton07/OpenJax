use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn core_event_to_ui_state_mapping() {
    let mut app = App::default();

    app.handle_event(AppEvent::CoreEvent(Event::ToolCallStarted {
        turn_id: 1,
        tool_name: "read_file".to_string(),
    }));
    app.handle_event(AppEvent::CoreEvent(Event::AssistantMessage {
        turn_id: 1,
        content: "done".to_string(),
    }));
    app.handle_event(AppEvent::CoreEvent(Event::ToolCallCompleted {
        turn_id: 1,
        tool_name: "read_file".to_string(),
        ok: true,
        output: "ok".to_string(),
    }));

    assert_eq!(app.state.messages.len(), 3);
    assert!(app.state.messages[0].content.contains("tool started"));
    assert_eq!(app.state.messages[1].role, "assistant");
    assert_eq!(app.state.messages[1].content, "done");
    assert!(app.state.messages[2].content.contains("tool completed"));
}
