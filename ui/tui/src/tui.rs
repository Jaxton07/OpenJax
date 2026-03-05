use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::history_cell::HistoryCell;
use crate::insert_history::insert_history_lines;
use crate::terminal::{self, CrosstermTerminal};

pub struct Tui {
    terminal: CrosstermTerminal,
    pending_history_lines: Vec<Line<'static>>,
    has_rendered_history: bool,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let viewport = std::env::var("OPENJAX_TUI_INLINE_HEIGHT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .map(|h| h.clamp(8, 60))
            .unwrap_or(16);
        let mut terminal = terminal::init_crossterm_terminal(viewport)?;
        terminal.hide_cursor()?;
        Ok(Self {
            terminal,
            pending_history_lines: Vec::new(),
            has_rendered_history: false,
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

    pub fn draw<F>(
        &mut self,
        desired_height: u16,
        status_line: Option<Line<'static>>,
        input_line: Line<'static>,
        input_cursor: u16,
        approval_lines: Option<Vec<Line<'static>>>,
        footer_text: String,
        mut render_live: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(Rect, &mut ratatui::buffer::Buffer),
    {
        let screen = self.terminal.size()?;
        let current_area = self.terminal.area();
        let plan = compute_viewport_plan(current_area, screen.width, screen.height, desired_height);
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
            let approval_height = approval_lines.as_ref().map_or(0, |l| l.len() as u16);
            let constraints = layout_constraints(approval_height, status_line.is_some());
            let status_idx = if status_line.is_some() {
                Some(1usize)
            } else {
                None
            };
            let input_idx = if status_line.is_some() { 2 } else { 1 };
            let chunks = Layout::vertical(constraints).split(draw_area);

            render_live(chunks[0], frame.buffer_mut());

            if let Some(idx) = status_idx
                && let Some(status_line) = status_line.clone()
            {
                frame.render_widget(Clear, chunks[idx]);
                let status_widget = Paragraph::new(status_line);
                status_widget.render(chunks[idx], frame.buffer_mut());
            }

            frame.render_widget(Clear, chunks[input_idx]);
            let input_widget = Paragraph::new(input_line.clone()).block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)),
            );
            input_widget.render(chunks[input_idx], frame.buffer_mut());
            let cursor_x = chunks[input_idx]
                .x
                .saturating_add(input_cursor)
                .min(chunks[input_idx].x + chunks[input_idx].width.saturating_sub(1));
            frame.set_cursor_position((cursor_x, chunks[input_idx].y + 1));

            let footer_idx = if approval_height > 0 {
                input_idx + 4
            } else {
                input_idx + 1
            };
            if approval_height > 0 {
                frame.render_widget(Clear, chunks[input_idx + 1]);
                let approval_area = chunks[input_idx + 2];
                frame.render_widget(Clear, approval_area);
                let approval_widget = Paragraph::new(approval_lines.unwrap_or_default());
                approval_widget.render(approval_area, frame.buffer_mut());
                frame.render_widget(Clear, chunks[input_idx + 3]);
            }

            frame.render_widget(Clear, chunks[footer_idx]);
            let footer = Paragraph::new(Line::from(vec![Span::styled(
                footer_text,
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            )]));
            footer.render(chunks[footer_idx], frame.buffer_mut());
        })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ViewportPlan {
    area: Rect,
    scroll_up: u16,
}

fn compute_viewport_plan(
    current_area: Rect,
    screen_width: u16,
    screen_height: u16,
    desired_height: u16,
) -> ViewportPlan {
    let bounded_height = desired_height.clamp(8, screen_height.max(8));
    let mut area = current_area;
    area.width = screen_width;
    area.height = bounded_height.min(screen_height);

    let mut scroll_up = 0;
    if area.bottom() > screen_height {
        scroll_up = area.bottom().saturating_sub(screen_height);
        area.y = screen_height.saturating_sub(area.height);
    }

    ViewportPlan { area, scroll_up }
}

fn layout_constraints(approval_height: u16, status_visible: bool) -> Vec<Constraint> {
    let mut constraints = vec![Constraint::Min(2)];
    if status_visible {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(2));
    if approval_height > 0 {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(approval_height));
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    constraints
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Constraint;
    use ratatui::layout::Rect;

    use super::{ViewportPlan, compute_viewport_plan, layout_constraints};

    #[test]
    fn viewport_stays_near_current_cursor_when_space_is_available() {
        let current = Rect::new(0, 10, 0, 0);
        let next = compute_viewport_plan(current, 100, 40, 12);
        assert_eq!(
            next,
            ViewportPlan {
                area: Rect::new(0, 10, 100, 12),
                scroll_up: 0,
            }
        );
    }

    #[test]
    fn viewport_overflow_requests_scroll_and_clamps_to_bottom() {
        let current = Rect::new(0, 25, 0, 0);
        let next = compute_viewport_plan(current, 100, 30, 12);
        assert_eq!(
            next,
            ViewportPlan {
                area: Rect::new(0, 18, 100, 12),
                scroll_up: 7,
            }
        );
    }

    #[test]
    fn layout_places_status_row_above_input() {
        let constraints = layout_constraints(0, true);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(2),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }
}
