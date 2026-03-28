use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Eq, PartialEq)]
pub enum InputAction {
    None,
    Quit,
    Submit,
    AcceptSuggestion,
    Backspace,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Append(String),
    DismissOverlay,
}

pub fn map_event(event: Event) -> InputAction {
    match event {
        Event::Paste(text) => InputAction::Append(text),
        Event::Key(key) => map_key(key),
        _ => InputAction::None,
    }
}

fn map_key(key: KeyEvent) -> InputAction {
    if key.kind == KeyEventKind::Release {
        return InputAction::None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return InputAction::Quit;
    }
    match key.code {
        KeyCode::Enter => InputAction::Submit,
        KeyCode::Tab => InputAction::AcceptSuggestion,
        KeyCode::Backspace => InputAction::Backspace,
        KeyCode::Left => InputAction::MoveLeft,
        KeyCode::Right => InputAction::MoveRight,
        KeyCode::Up => InputAction::MoveUp,
        KeyCode::Down => InputAction::MoveDown,
        KeyCode::Esc => InputAction::DismissOverlay,
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            InputAction::Append(c.to_string())
        }
        KeyCode::Char(_) => InputAction::None,
        _ => InputAction::None,
    }
}
