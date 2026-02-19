use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use openjax_tui::app_event::AppEvent;
use openjax_tui::tui::map_crossterm_event;

#[test]
fn keymap_maps_primary_shortcuts() {
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE
        ))),
        Some(AppEvent::SubmitInput)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE
        ))),
        Some(AppEvent::ToggleHelp)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        ))),
        Some(AppEvent::Quit)
    ));
    assert!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))).is_none()
    );
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        ))),
        Some(AppEvent::InputChar('q'))
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))),
        Some(AppEvent::MoveCursorLeft)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE
        ))),
        Some(AppEvent::MoveCursorRight)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))),
        Some(AppEvent::HistoryPrev)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))),
        Some(AppEvent::HistoryNext)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::PageUp,
            KeyModifiers::NONE
        ))),
        Some(AppEvent::ScrollPageUp)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::PageDown,
            KeyModifiers::NONE
        ))),
        Some(AppEvent::ScrollPageDown)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE))),
        Some(AppEvent::ScrollTop)
    ));
    assert!(matches!(
        map_crossterm_event(Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))),
        Some(AppEvent::ScrollBottom)
    ));
}
