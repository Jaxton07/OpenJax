use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn input_state_and_message_append_order() {
    let mut app = App::default();

    app.handle_event(AppEvent::InputChar('h'));
    app.handle_event(AppEvent::InputChar('i'));
    assert_eq!(app.state.input, "hi");

    app.handle_event(AppEvent::Backspace);
    assert_eq!(app.state.input, "h");

    app.handle_event(AppEvent::SubmitInput);
    assert_eq!(app.state.input, "");
    assert_eq!(app.state.messages.len(), 1);
    assert_eq!(app.state.messages[0].role, "user");
    assert_eq!(app.state.messages[0].content, "h");
}
