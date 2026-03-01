use openjax_tui::custom_terminal::TerminalState;
use openjax_tui::insert_history::insert_history_lines;
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[test]
fn insert_history_lines_updates_viewport_and_writes_bytes() {
    let mut state = TerminalState::new(Size::new(80, 20), Position::new(0, 10));
    state.set_viewport_area(Rect::new(0, 10, 80, 8));

    let lines = vec![
        Line::from(Span::styled("header", Style::default().fg(Color::Cyan))),
        Line::from("body line"),
    ];

    let mut out = Vec::<u8>::new();
    insert_history_lines(&mut out, &mut state, lines).expect("insert history succeeds");

    assert!(!out.is_empty(), "history insert should emit ansi output");
    assert!(
        state.viewport_area.y >= 10,
        "viewport top should not move upward after insertion"
    );
}
