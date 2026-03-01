use crossterm::cursor::MoveTo;
use crossterm::queue;
use ratatui::backend::Backend;
use ratatui::layout::{Position, Rect, Size};
use std::io;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct TerminalState {
    pub viewport_area: Rect,
    pub last_known_screen_size: Size,
    pub last_known_cursor_pos: Position,
}

impl TerminalState {
    pub fn new(screen_size: Size, cursor_pos: Position) -> Self {
        Self {
            viewport_area: Rect::new(0, cursor_pos.y, screen_size.width, 0),
            last_known_screen_size: screen_size,
            last_known_cursor_pos: cursor_pos,
        }
    }

    pub fn set_viewport_area(&mut self, area: Rect) {
        self.viewport_area = area;
    }

    pub fn update_from_backend_size(&mut self, size: Size) {
        self.last_known_screen_size = size;
    }

    pub fn update_cursor_from_backend<B: Backend>(&mut self, backend: &mut B) {
        match backend.get_cursor_position() {
            Ok(pos) => self.last_known_cursor_pos = pos,
            Err(err) => tracing::debug!("failed to read cursor position: {err}"),
        }
    }

    pub fn restore_cursor<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        queue!(
            writer,
            MoveTo(self.last_known_cursor_pos.x, self.last_known_cursor_pos.y)
        )?;
        Ok(())
    }
}
