use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Eq, PartialEq)]
pub enum InputAction {
    None,
    Quit,
    Submit,
    Backspace,
    Append(String),
    Clear,
}

pub fn map_event(event: Event) -> InputAction {
    match event {
        Event::Paste(text) => InputAction::Append(text),
        Event::Key(key) => map_key(key),
        _ => InputAction::None,
    }
}

fn map_key(key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return InputAction::Quit;
    }
    match key.code {
        KeyCode::Enter => InputAction::Submit,
        KeyCode::Backspace => InputAction::Backspace,
        KeyCode::Esc => InputAction::Clear,
        KeyCode::Char(c) => InputAction::Append(c.to_string()),
        _ => InputAction::None,
    }
}
