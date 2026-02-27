use openjax_protocol::Event;
use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn core_event_to_ui_state_mapping() {
    let mut app = App::default();

    app.handle_event(AppEvent::CoreEvent(Event::ToolCallStarted {
        turn_id: 1,
        tool_name: "read_file".to_string(),
        target: Some("a.txt".to_string()),
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

    assert_eq!(app.state.transcript.messages.len(), 2);
    assert_eq!(app.state.transcript.messages[0].role, "assistant");
    assert_eq!(app.state.transcript.messages[0].content, "done");
    assert_eq!(app.state.transcript.messages[1].role, "tool");
    assert_eq!(
        app.state.transcript.messages[1].target.as_deref(),
        Some("a.txt")
    );
}

#[test]
fn core_event_mapping_can_show_system_messages_when_enabled() {
    let mut app = App::default();
    app.state.show_system_messages = true;

    app.handle_event(AppEvent::CoreEvent(Event::ToolCallStarted {
        turn_id: 1,
        tool_name: "read_file".to_string(),
        target: Some("a.txt".to_string()),
    }));

    assert_eq!(app.state.transcript.messages.len(), 1);
    assert_eq!(app.state.transcript.messages[0].role, "system");
    assert!(
        app.state.transcript.messages[0]
            .content
            .contains("tool started")
    );
}
