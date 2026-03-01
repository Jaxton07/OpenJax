use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::custom_terminal::{self, CrosstermTerminal};
use crate::history_cell::HistoryCell;
use crate::insert_history::insert_history_lines;

pub struct Tui {
    terminal: CrosstermTerminal,
    pending_history_lines: Vec<Line<'static>>,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let viewport = std::env::var("OPENJAX_TUI_INLINE_HEIGHT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .map(|h| h.clamp(8, 60))
            .unwrap_or(16);
        let mut terminal = custom_terminal::init_crossterm_terminal(viewport)?;
        terminal.hide_cursor()?;
        Ok(Self {
            terminal,
            pending_history_lines: Vec::new(),
        })
    }

    pub fn queue_history_cells(&mut self, cells: Vec<HistoryCell>) {
        if cells.is_empty() {
            return;
        }
        for cell in cells {
            self.pending_history_lines.extend(cell.lines);
        }
    }

    pub fn viewport_size(&self) -> Rect {
        self.terminal.area()
    }

    pub fn draw<F>(
        &mut self,
        desired_height: u16,
        input_line: Line<'static>,
        input_cursor: u16,
        footer_text: String,
        mut render_live: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(Rect, &mut ratatui::buffer::Buffer),
    {
        let screen = self.terminal.size()?;
        let current_area = self.terminal.area();
        let bounded_height = desired_height.clamp(8, screen.height.max(8));
        let viewport = Rect::new(
            current_area.x,
            screen.height.saturating_sub(bounded_height),
            screen.width,
            bounded_height,
        );
        self.terminal.set_viewport_area(viewport);

        if !self.pending_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.pending_history_lines);
            insert_history_lines(&mut self.terminal, lines)?;
            self.terminal.update_cursor_from_backend();
        }

        self.terminal.draw(|frame| {
            let draw_area = frame.area();
            frame.render_widget(Clear, draw_area);
            let chunks = Layout::vertical([
                Constraint::Min(2),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(draw_area);

            render_live(chunks[0], frame.buffer_mut());

            frame.render_widget(Clear, chunks[1]);
            let input_widget =
                Paragraph::new(input_line.clone()).block(Block::default().borders(Borders::TOP));
            input_widget.render(chunks[1], frame.buffer_mut());
            let cursor_x = chunks[1]
                .x
                .saturating_add(input_cursor)
                .min(chunks[1].x + chunks[1].width.saturating_sub(1));
            frame.set_cursor_position((cursor_x, chunks[1].y + 1));

            frame.render_widget(Clear, chunks[2]);
            let footer = Paragraph::new(Line::from(vec![Span::styled(
                footer_text,
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            )]));
            footer.render(chunks[2], frame.buffer_mut());
        })?;

        Ok(())
    }
}
