use ratatui::layout::{Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::{BottomLayout, TransientPanel};
use crate::history_cell::HistoryCell;
use crate::insert_history::insert_history_lines;
use crate::terminal::{self, CrosstermTerminal};
use crate::viewport::{
    build_chunk_indices, clamp_transient_rows, clip_transient_lines, compute_viewport_plan,
    layout_constraints, stable_bottom_chrome_height,
};

pub struct Tui {
    terminal: CrosstermTerminal,
    pending_history_lines: Vec<Line<'static>>,
    has_rendered_history: bool,
    last_bottom_chrome_height: u16,
}

pub struct DrawRequest {
    pub desired_height: u16,
    pub bottom_layout: BottomLayout,
    pub reset_sticky_height: bool,
    pub status_line: Option<Line<'static>>,
    pub input_line: Line<'static>,
    pub input_cursor: u16,
    pub transient_panel: Option<TransientPanel>,
    pub footer_text: String,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let mut terminal = terminal::init_crossterm_terminal()?;
        terminal.hide_cursor()?;
        Ok(Self {
            terminal,
            pending_history_lines: Vec::new(),
            has_rendered_history: false,
            last_bottom_chrome_height: 3,
        })
    }

    pub fn queue_history_cells(&mut self, cells: Vec<HistoryCell>) {
        if cells.is_empty() {
            return;
        }
        for cell in cells {
            if matches!(cell.role, crate::history_cell::CellRole::Banner) {
                self.has_rendered_history = false;
            }
            if self.has_rendered_history {
                self.pending_history_lines.push(Line::default());
            }
            self.pending_history_lines.extend(cell.lines);
            self.has_rendered_history = true;
        }
    }

    pub fn viewport_size(&self) -> Rect {
        self.terminal.area()
    }

    pub fn draw<F>(&mut self, request: DrawRequest, mut render_live: F) -> anyhow::Result<()>
    where
        F: FnMut(Rect, &mut ratatui::buffer::Buffer),
    {
        let DrawRequest {
            desired_height,
            bottom_layout,
            reset_sticky_height,
            status_line,
            input_line,
            input_cursor,
            transient_panel,
            footer_text,
        } = request;
        let screen = self.terminal.size()?;
        let current_area = self.terminal.area();
        let current_bottom_chrome_height = stable_bottom_chrome_height(bottom_layout);
        let bottom_chrome_growth =
            current_bottom_chrome_height.saturating_sub(self.last_bottom_chrome_height);
        // Normally the viewport only grows (sticky height) so live content stays visible
        // while streaming.  When reset_sticky_height is set (e.g. turn completed) we allow
        // it to shrink back to the actual desired height, eliminating the blank-line gap
        // that otherwise appears after streaming output is committed to history.
        // terminal.clear() uses ClearType::AfterCursor which erases from the viewport top
        // to end-of-screen, so stale rows below the new smaller viewport are cleaned up
        // automatically when plan.area != current_area triggers the clear call below.
        let desired_with_reserve = if reset_sticky_height {
            desired_height
        } else {
            desired_height.max(current_area.height.saturating_add(bottom_chrome_growth))
        };
        let plan = compute_viewport_plan(
            current_area,
            screen.width,
            screen.height,
            desired_with_reserve,
        );
        self.last_bottom_chrome_height = current_bottom_chrome_height;
        if plan.scroll_up > 0 && current_area.top() > 0 {
            self.terminal
                .scroll_region_up(0..current_area.top(), plan.scroll_up)?;
        }
        if plan.area != current_area {
            self.terminal.clear()?;
            self.terminal.set_viewport_area(plan.area);
        }

        if !self.pending_history_lines.is_empty() {
            let lines = std::mem::take(&mut self.pending_history_lines);
            insert_history_lines(&mut self.terminal, lines)?;
            self.terminal.update_cursor_from_backend();
        }

        self.terminal.draw(|frame| {
            let draw_area = frame.area();
            frame.render_widget(Clear, draw_area);
            let transient_rows = clamp_transient_rows(draw_area.height, bottom_layout);
            let constraints = layout_constraints(bottom_layout, transient_rows);
            let idx = build_chunk_indices(bottom_layout, transient_rows);
            let chunks = Layout::vertical(constraints).split(draw_area);

            render_live(chunks[idx.live], frame.buffer_mut());

            if let Some(i) = idx.status
                && let Some(status_line) = status_line.clone()
            {
                frame.render_widget(Clear, chunks[i]);
                let status_widget = Paragraph::new(status_line);
                status_widget.render(chunks[i], frame.buffer_mut());
            }

            if let Some(i) = idx.transient {
                let transient_area = chunks[i];
                frame.render_widget(Clear, transient_area);
                if let Some(panel) = transient_panel.as_ref() {
                    let clipped = clip_transient_lines(
                        panel.kind,
                        &panel.lines,
                        panel.selected_index,
                        transient_area.height as usize,
                    );
                    if !clipped.is_empty() {
                        let panel_widget = Paragraph::new(clipped);
                        panel_widget.render(transient_area, frame.buffer_mut());
                    }
                }
            }

            frame.render_widget(Clear, chunks[idx.input]);
            let input_widget = Paragraph::new(input_line.clone()).block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)),
            );
            input_widget.render(chunks[idx.input], frame.buffer_mut());
            let cursor_x = chunks[idx.input]
                .x
                .saturating_add(input_cursor)
                .min(chunks[idx.input].x + chunks[idx.input].width.saturating_sub(1));
            frame.set_cursor_position((cursor_x, chunks[idx.input].y + 1));

            frame.render_widget(Clear, chunks[idx.footer]);
            let footer = Paragraph::new(Line::from(vec![Span::styled(
                footer_text,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )]));
            footer.render(chunks[idx.footer], frame.buffer_mut());
        })?;

        Ok(())
    }
}
