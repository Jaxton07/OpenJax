use std::io;
use std::io::Stdout;
use std::io::Write;

use std::ops::Range;

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::terminal::Clear;
use crossterm::terminal::ScrollUp;
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect, Size};
use ratatui::widgets::Widget;

use crate::insert_history::{ResetScrollRegion, SetScrollRegion};

use super::diff::diff_buffers;
use super::draw::draw;

#[derive(Debug, Hash)]
pub struct Frame<'a> {
    pub(crate) cursor_position: Option<Position>,
    pub(crate) viewport_area: Rect,
    pub(crate) buffer: &'a mut Buffer,
}

impl Frame<'_> {
    pub const fn area(&self) -> Rect {
        self.viewport_area
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }

    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Terminal<B>
where
    B: Backend + Write,
{
    backend: B,
    buffers: [Buffer; 2],
    current: usize,
    pub hidden_cursor: bool,
    pub viewport_area: Rect,
    pub last_known_screen_size: Size,
    pub last_known_cursor_pos: Position,
}

impl<B> Drop for Terminal<B>
where
    B: Backend + Write,
{
    #[allow(clippy::print_stderr)]
    fn drop(&mut self) {
        if self.hidden_cursor
            && let Err(err) = self.show_cursor()
        {
            eprintln!("Failed to show the cursor: {err}");
        }
    }
}

impl<B> Terminal<B>
where
    B: Backend + Write,
{
    pub fn with_options(mut backend: B) -> io::Result<Self> {
        let screen_size = backend.size()?;
        let cursor_pos = backend.get_cursor_position().unwrap_or_else(|err| {
            tracing::warn!("failed to read initial cursor position; defaulting to origin: {err}");
            Position { x: 0, y: 0 }
        });
        Ok(Self {
            backend,
            buffers: [Buffer::empty(Rect::ZERO), Buffer::empty(Rect::ZERO)],
            current: 0,
            hidden_cursor: false,
            viewport_area: Rect::new(0, cursor_pos.y, 0, 0),
            last_known_screen_size: screen_size,
            last_known_cursor_pos: cursor_pos,
        })
    }

    pub fn get_frame(&mut self) -> Frame<'_> {
        let viewport_area = self.viewport_area;
        let current = self.current;
        Frame {
            cursor_position: None,
            viewport_area,
            buffer: &mut self.buffers[current],
        }
    }

    fn current_buffer(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    fn previous_buffer(&self) -> &Buffer {
        &self.buffers[1 - self.current]
    }

    fn previous_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[1 - self.current]
    }

    pub const fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let updates = diff_buffers(self.previous_buffer(), self.current_buffer());
        let last_put_command = updates.iter().rfind(|command| command.is_put());
        if let Some(super::diff::DrawCommand::Put { x, y, .. }) = last_put_command {
            self.last_known_cursor_pos = Position { x: *x, y: *y };
        }
        draw(&mut self.backend, updates.into_iter())
    }

    pub fn resize(&mut self, screen_size: Size) -> io::Result<()> {
        self.last_known_screen_size = screen_size;
        Ok(())
    }

    pub fn set_viewport_area(&mut self, area: Rect) {
        self.current_buffer_mut().resize(area);
        self.previous_buffer_mut().resize(area);
        self.viewport_area = area;
    }

    pub fn scroll_region_up(&mut self, region: Range<u16>, scroll_by: u16) -> io::Result<()> {
        if scroll_by == 0 || region.start >= region.end {
            return Ok(());
        }
        let last_cursor_pos = self.last_known_cursor_pos;
        // DECSTBM uses 1-based inclusive bounds; `region` end here is already screen-row upper bound.
        queue!(
            self.backend,
            SetScrollRegion(region.start + 1..region.end),
            MoveTo(0, region.end.saturating_sub(1)),
            ScrollUp(scroll_by),
            ResetScrollRegion,
            MoveTo(last_cursor_pos.x, last_cursor_pos.y)
        )?;
        std::io::Write::flush(&mut self.backend)?;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    pub fn autoresize(&mut self) -> io::Result<()> {
        let screen_size = self.size()?;
        if screen_size != self.last_known_screen_size {
            self.resize(screen_size)?;
        }
        Ok(())
    }

    pub fn draw<F>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.try_draw(|frame| {
            render_callback(frame);
            io::Result::Ok(())
        })
    }

    pub fn try_draw<F, E>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame) -> Result<(), E>,
        E: Into<io::Error>,
    {
        self.autoresize()?;

        let mut frame = self.get_frame();
        render_callback(&mut frame).map_err(Into::into)?;
        let cursor_position = frame.cursor_position;

        self.flush()?;

        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }

        self.swap_buffers();
        Backend::flush(&mut self.backend)?;

        Ok(())
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        self.hidden_cursor = true;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()?;
        self.hidden_cursor = false;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.backend.get_cursor_position()
    }

    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.backend.set_cursor_position(position)?;
        self.last_known_cursor_pos = position;
        Ok(())
    }

    pub fn clear(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        self.backend.clear_region(ClearType::AfterCursor)?;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    pub fn clear_all(&mut self) -> io::Result<()> {
        self.backend.set_cursor_position(Position { x: 0, y: 0 })?;
        self.backend.clear_region(ClearType::All)?;
        self.previous_buffer_mut().reset();
        self.current_buffer_mut().reset();
        self.last_known_cursor_pos = Position { x: 0, y: 0 };
        Ok(())
    }

    pub fn clear_scrollback(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        queue!(self.backend, Clear(crossterm::terminal::ClearType::Purge))?;
        std::io::Write::flush(&mut self.backend)?;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    pub fn swap_buffers(&mut self) {
        self.previous_buffer_mut().reset();
        self.current = 1 - self.current;
    }

    pub fn size(&self) -> io::Result<Size> {
        self.backend.size()
    }

    pub fn area(&self) -> Rect {
        self.viewport_area
    }

    pub fn update_cursor_from_backend(&mut self) {
        if let Ok(pos) = self.backend.get_cursor_position() {
            self.last_known_cursor_pos = pos;
        }
    }
}

pub type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn init_crossterm_terminal(_inline_height: u16) -> io::Result<CrosstermTerminal> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::with_options(backend)?;
    let size = terminal.size()?;
    let viewport = initial_viewport_rect(size.height, terminal.last_known_cursor_pos.y);
    terminal.set_viewport_area(viewport);
    Ok(terminal)
}

fn initial_viewport_rect(screen_height: u16, cursor_y: u16) -> Rect {
    let max_y = screen_height.saturating_sub(1);
    let y = cursor_y.min(max_y);
    Rect::new(0, y, 0, 0)
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::initial_viewport_rect;

    #[test]
    fn initial_viewport_anchors_to_cursor_row() {
        let viewport = initial_viewport_rect(40, 12);
        assert_eq!(viewport, Rect::new(0, 12, 0, 0));
    }

    #[test]
    fn initial_viewport_clamps_when_cursor_near_bottom() {
        let viewport = initial_viewport_rect(30, 42);
        assert_eq!(viewport, Rect::new(0, 29, 0, 0));
    }
}
