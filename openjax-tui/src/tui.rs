use crossterm::event::{Event, KeyCode, KeyEvent, read};

use crate::app_event::AppEvent;

pub fn map_crossterm_event(event: Event) -> Option<AppEvent> {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            ..
        }) => Some(AppEvent::Quit),
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            ..
        }) => Some(AppEvent::SubmitInput),
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            ..
        }) => Some(AppEvent::Backspace),
        Event::Key(KeyEvent {
            code: KeyCode::Char(ch),
            ..
        }) => Some(AppEvent::InputChar(ch)),
        _ => None,
    }
}

pub fn next_app_event() -> anyhow::Result<Option<AppEvent>> {
    let event = read()?;
    Ok(map_crossterm_event(event))
}
