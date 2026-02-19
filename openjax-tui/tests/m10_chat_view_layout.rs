use openjax_tui::state::{AppState, UiMessage};
use openjax_tui::ui::chat_view;

#[test]
fn multiline_assistant_message_has_no_prefix_padding_on_following_lines() {
    let mut state = AppState::default();
    state.messages.push(UiMessage {
        role: "assistant".to_string(),
        content: "first\nsecond".to_string(),
    });

    let lines = chat_view::render_lines(&state);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].to_string(), "[assistant] first");
    assert_eq!(lines[1].to_string(), "second");
}
