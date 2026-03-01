use openjax_tui::custom_terminal::TerminalState;
use openjax_tui::insert_history::insert_history_lines;
use ratatui::layout::{Position, Rect, Size};
use ratatui::text::Line;

#[test]
fn insert_history_restores_cursor_to_latest_terminal_state_position() {
    let mut state = TerminalState::new(Size::new(80, 24), Position::new(11, 3));
    state.set_viewport_area(Rect::new(0, 12, 80, 10));
    state.last_known_cursor_pos = Position::new(7, 5);

    let mut out = Vec::<u8>::new();
    insert_history_lines(&mut out, &mut state, vec![Line::from("line")])
        .expect("history insertion should succeed");

    let ansi = String::from_utf8_lossy(&out);
    assert!(
        ansi.contains("\u{1b}[6;8H"),
        "expected cursor restore escape sequence for (x=7,y=5)"
    );
}
