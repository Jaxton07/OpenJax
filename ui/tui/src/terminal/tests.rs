use std::io;
use std::io::Write;

use crate::insert_history::insert_history_lines;
use pretty_assertions::assert_eq;
use ratatui::backend::Backend;
use ratatui::backend::WindowSize;
use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::text::Line;

use super::Terminal;
use super::diff::{DrawCommand, diff_buffers};

#[derive(Debug)]
struct RecordingBackend {
    output: Vec<u8>,
    size: Size,
    cursor: Position,
}

impl RecordingBackend {
    fn new(size: Size, cursor: Position) -> Self {
        Self {
            output: Vec::new(),
            size,
            cursor,
        }
    }
}

impl Write for RecordingBackend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Backend for RecordingBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for _ in content {}
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = position.into();
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn diff_buffers_does_not_emit_clear_to_end_for_full_width_row() {
    let area = Rect::new(0, 0, 3, 2);
    let previous = Buffer::empty(area);
    let mut next = Buffer::empty(area);

    next.cell_mut((2, 0))
        .expect("cell should exist")
        .set_symbol("X");

    let commands = diff_buffers(&previous, &next);

    let clear_count = commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::ClearToEnd { y, .. } if *y == 0))
        .count();
    assert_eq!(
        0, clear_count,
        "expected diff_buffers not to emit ClearToEnd; commands: {commands:?}",
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Put { x: 2, y: 0, .. })),
        "expected diff_buffers to update the final cell; commands: {commands:?}",
    );
}

#[test]
fn diff_buffers_clear_to_end_starts_after_wide_char() {
    let area = Rect::new(0, 0, 10, 1);
    let mut previous = Buffer::empty(area);
    let mut next = Buffer::empty(area);

    previous.set_string(0, 0, "中文", Style::default());
    next.set_string(0, 0, "中", Style::default());

    let commands = diff_buffers(&previous, &next);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, DrawCommand::ClearToEnd { x: 2, y: 0, .. })),
        "expected clear-to-end to start after the remaining wide char; commands: {commands:?}"
    );
}

#[test]
fn diff_buffers_clears_from_column_zero_when_row_becomes_empty() {
    let area = Rect::new(0, 0, 6, 1);
    let mut previous = Buffer::empty(area);
    let next = Buffer::empty(area);

    previous.set_string(0, 0, "› test", Style::default());

    let commands = diff_buffers(&previous, &next);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, DrawCommand::ClearToEnd { x: 0, y: 0, .. })),
        "expected clear-to-end from x=0 for blank row; commands: {commands:?}"
    );
}

#[test]
fn insert_history_lines_clamps_scroll_region_when_viewport_starts_at_top() {
    let backend = RecordingBackend::new(Size::new(10, 4), Position { x: 2, y: 1 });
    let mut terminal = Terminal::with_options(backend).expect("terminal should initialize");
    let viewport = Rect::new(0, 0, 10, 4);
    terminal.set_viewport_area(viewport);
    let last_known_cursor_pos = terminal.last_known_cursor_pos;

    insert_history_lines(&mut terminal, vec![Line::from("hello")])
        .expect("history insertion should succeed");

    let output = {
        let backend = terminal.backend_mut();
        String::from_utf8_lossy(&backend.output).to_string()
    };
    assert!(
        output.contains("\u{1b}[1;1r"),
        "expected a valid scroll region when viewport.top() == 0; output: {output:?}"
    );
    assert!(
        !output.contains("\u{1b}[1;0r"),
        "unexpected invalid scroll region emitted; output: {output:?}"
    );
    assert_eq!(terminal.viewport_area, viewport);
    assert_eq!(terminal.last_known_cursor_pos, last_known_cursor_pos);
}
