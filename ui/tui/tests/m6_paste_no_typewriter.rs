use crossterm::event::Event;
use tui_next::input::{InputAction, map_event};

#[test]
fn paste_event_is_single_append_action() {
    let action = map_event(Event::Paste("abcdef".to_string()));
    assert_eq!(action, InputAction::Append("abcdef".to_string()));
}
