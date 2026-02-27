use openjax_tui::app::App;
use openjax_tui::app_event::AppEvent;

#[test]
fn input_state_and_message_append_order() {
    let mut app = App::default();

    app.handle_event(AppEvent::InputChar('h'));
    app.handle_event(AppEvent::InputChar('i'));
    assert_eq!(app.state.input_state.buffer, "hi");

    app.handle_event(AppEvent::Backspace);
    assert_eq!(app.state.input_state.buffer, "h");

    app.handle_event(AppEvent::SubmitInput);
    assert_eq!(app.state.input_state.buffer, "");
    assert_eq!(app.state.transcript.messages.len(), 1);
    assert_eq!(app.state.transcript.messages[0].role, "user");
    assert_eq!(app.state.transcript.messages[0].content, "h");
}

#[test]
fn cursor_edit_and_history_navigation_work() {
    let mut app = App::default();

    app.handle_event(AppEvent::InputChar('a'));
    app.handle_event(AppEvent::InputChar('c'));
    app.handle_event(AppEvent::MoveCursorLeft);
    app.handle_event(AppEvent::InputChar('b'));
    assert_eq!(app.state.input_state.buffer, "abc");

    app.handle_event(AppEvent::SubmitInput);
    assert_eq!(app.state.transcript.messages.len(), 1);
    assert_eq!(app.state.transcript.messages[0].content, "abc");

    app.handle_event(AppEvent::InputChar('x'));
    app.handle_event(AppEvent::SubmitInput);
    assert_eq!(app.state.transcript.messages.len(), 2);
    assert_eq!(app.state.transcript.messages[1].content, "x");

    app.handle_event(AppEvent::HistoryPrev);
    assert_eq!(app.state.input_state.buffer, "x");

    app.handle_event(AppEvent::HistoryPrev);
    assert_eq!(app.state.input_state.buffer, "abc");

    app.handle_event(AppEvent::HistoryNext);
    assert_eq!(app.state.input_state.buffer, "x");
}

#[test]
fn chat_scroll_controls_work() {
    let mut app = App::default();
    assert!(app.state.transcript.follow_output);
    app.state.transcript.chat_scroll = 30;
    app.state.transcript.follow_output = false;

    app.handle_event(AppEvent::ScrollPageUp);
    assert_eq!(app.state.transcript.chat_scroll, 20);
    assert!(!app.state.transcript.follow_output);

    app.handle_event(AppEvent::ScrollTop);
    assert_eq!(app.state.transcript.chat_scroll, 0);
    assert!(!app.state.transcript.follow_output);

    app.handle_event(AppEvent::ScrollBottom);
    assert!(app.state.transcript.follow_output);
    assert_eq!(app.state.transcript.chat_scroll, usize::MAX);
}
