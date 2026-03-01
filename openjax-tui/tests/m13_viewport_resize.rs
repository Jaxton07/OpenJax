use openjax_tui::custom_terminal::TerminalState;
use ratatui::layout::{Position, Rect, Size};

#[test]
fn terminal_state_tracks_resize_and_viewport_area() {
    let mut state = TerminalState::new(Size::new(100, 30), Position::new(0, 3));
    assert_eq!(state.last_known_screen_size, Size::new(100, 30));

    state.update_from_backend_size(Size::new(120, 40));
    assert_eq!(state.last_known_screen_size, Size::new(120, 40));

    state.set_viewport_area(Rect::new(0, 25, 120, 15));
    assert_eq!(state.viewport_area, Rect::new(0, 25, 120, 15));
}
