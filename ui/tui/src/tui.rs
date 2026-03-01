use std::io::stdout;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::custom_terminal::CustomTerminal;
use crate::history_cell::HistoryCell;
use crate::insert_history::insert_history_cells;

pub struct Tui {
    terminal: CustomTerminal,
    pending_history_cells: Vec<HistoryCell>,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let viewport = std::env::var("OPENJAX_TUI_INLINE_HEIGHT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .map(|h| h.clamp(8, 60))
            .unwrap_or(16);
        Ok(Self {
            terminal: CustomTerminal::new(stdout(), viewport)?,
            pending_history_cells: Vec::new(),
        })
    }

    pub fn queue_history_cells(&mut self, cells: Vec<HistoryCell>) {
        if cells.is_empty() {
            return;
        }
        self.pending_history_cells.extend(cells);
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
        let area = self.terminal.area();
        let bounded_height = desired_height.min(area.height.max(8));
        let viewport = Rect {
            x: area.x,
            y: area.bottom().saturating_sub(bounded_height),
            width: area.width,
            height: bounded_height,
        };
        self.terminal.set_viewport_area(viewport);

        if !self.pending_history_cells.is_empty() {
            let cells = std::mem::take(&mut self.pending_history_cells);
            let cursor = self.terminal.last_known_cursor_pos;
            if viewport.height > 1 {
                insert_history_cells(self.terminal.backend_mut(), viewport, cursor, &cells)?;
            }
            self.terminal.update_cursor_from_backend();
        }

        self.terminal.draw(|frame, draw_area| {
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
                footer_text.clone(),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            )]));
            footer.render(chunks[2], frame.buffer_mut());
        })?;
        self.terminal.flush()?;
        Ok(())
    }
}
