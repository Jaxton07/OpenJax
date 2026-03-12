use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use std::collections::BTreeSet;

use crate::app::{BottomLayout, TransientKind, TransientPanel};
use crate::history_cell::HistoryCell;
use crate::insert_history::insert_history_lines;
use crate::terminal::{self, CrosstermTerminal};

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
        let _ = reset_sticky_height;
        let current_bottom_chrome_height = stable_bottom_chrome_height(bottom_layout);
        let bottom_chrome_growth =
            current_bottom_chrome_height.saturating_sub(self.last_bottom_chrome_height);
        let desired_with_reserve =
            desired_height.max(current_area.height.saturating_add(bottom_chrome_growth));
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

            let mut next_idx = 1usize;
            let status_idx = if bottom_layout.status_rows > 0 {
                let idx = next_idx;
                next_idx += 1;
                Some(idx)
            } else {
                None
            };
            let transient_idx = if transient_rows > 0 {
                let idx = next_idx;
                next_idx += 1;
                Some(idx)
            } else {
                None
            };
            let input_idx = next_idx;
            next_idx += 1;
            let footer_idx = next_idx;
            let chunks = Layout::vertical(constraints).split(draw_area);

            render_live(chunks[0], frame.buffer_mut());

            if let Some(idx) = status_idx
                && let Some(status_line) = status_line.clone()
            {
                frame.render_widget(Clear, chunks[idx]);
                let status_widget = Paragraph::new(status_line);
                status_widget.render(chunks[idx], frame.buffer_mut());
            }

            if let Some(idx) = transient_idx {
                let transient_area = chunks[idx];
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

            frame.render_widget(Clear, chunks[footer_idx]);
            let footer = Paragraph::new(Line::from(vec![Span::styled(
                footer_text,
                Style::default()
                    .fg(Color::DarkGray)
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

fn stable_bottom_chrome_height(layout: BottomLayout) -> u16 {
    layout
        .status_rows
        .saturating_add(layout.input_rows)
        .saturating_add(layout.footer_rows)
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

fn clamp_transient_rows(total_height: u16, layout: BottomLayout) -> u16 {
    if layout.transient_rows == 0 {
        return 0;
    }

    let fixed_rows = layout
        .status_rows
        .saturating_add(layout.input_rows)
        .saturating_add(layout.footer_rows);
    let max_transient = total_height.saturating_sub(fixed_rows.saturating_add(1));
    layout.transient_rows.min(max_transient)
}

fn layout_constraints(layout: BottomLayout, transient_rows: u16) -> Vec<Constraint> {
    let mut constraints = vec![Constraint::Min(1)];
    if layout.status_rows > 0 {
        constraints.push(Constraint::Length(layout.status_rows));
    }
    if transient_rows > 0 {
        constraints.push(Constraint::Length(transient_rows));
    }
    constraints.push(Constraint::Length(layout.input_rows));
    constraints.push(Constraint::Length(layout.footer_rows));
    constraints
}

fn clip_transient_lines(
    kind: TransientKind,
    lines: &[Line<'static>],
    selected_index: Option<usize>,
    max_rows: usize,
) -> Vec<Line<'static>> {
    if max_rows == 0 || lines.is_empty() {
        return Vec::new();
    }
    if lines.len() <= max_rows {
        return lines.to_vec();
    }

    match kind {
        TransientKind::Slash => {
            let selected = selected_index
                .unwrap_or(0)
                .min(lines.len().saturating_sub(1));
            let start = selected
                .saturating_sub(max_rows / 2)
                .min(lines.len().saturating_sub(max_rows));
            lines[start..start + max_rows].to_vec()
        }
        TransientKind::Approval => clip_approval_lines(lines, selected_index, max_rows),
        TransientKind::None => Vec::new(),
    }
}

fn clip_approval_lines(
    lines: &[Line<'static>],
    selected_index: Option<usize>,
    max_rows: usize,
) -> Vec<Line<'static>> {
    if max_rows == 0 || lines.is_empty() {
        return Vec::new();
    }

    let mut chosen: BTreeSet<usize> = BTreeSet::new();
    chosen.insert(0);

    if max_rows > 1 {
        if let Some(idx) = selected_index {
            chosen.insert(idx.min(lines.len().saturating_sub(1)));
        }
        let action_start = lines.len().saturating_sub(3);
        for idx in action_start..lines.len() {
            if chosen.len() >= max_rows {
                break;
            }
            chosen.insert(idx);
        }
        for idx in 1..action_start {
            if chosen.len() >= max_rows {
                break;
            }
            chosen.insert(idx);
        }
    }

    chosen
        .into_iter()
        .take(max_rows)
        .map(|idx| lines[idx].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::app::{BottomLayout, TransientKind};
    use ratatui::layout::Constraint;
    use ratatui::layout::Rect;
    use ratatui::text::Line;

    use super::{
        ViewportPlan, clamp_transient_rows, clip_transient_lines, compute_viewport_plan,
        layout_constraints, stable_bottom_chrome_height,
    };

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
        let layout = BottomLayout {
            status_rows: 1,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 0,
            transient_kind: TransientKind::None,
            footer_mode: crate::app::FooterMode::Idle,
        };
        let constraints = layout_constraints(layout, 0);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn layout_places_slash_palette_below_input() {
        let layout = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 3,
            transient_kind: TransientKind::Slash,
            footer_mode: crate::app::FooterMode::SlashActive,
        };
        let constraints = layout_constraints(layout, 3);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn layout_places_transient_region_between_input_and_footer() {
        let layout = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 5,
            transient_kind: TransientKind::Slash,
            footer_mode: crate::app::FooterMode::SlashActive,
        };
        let constraints = layout_constraints(layout, 5);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(5),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn stable_bottom_chrome_is_not_affected_by_transient_panels() {
        let slash = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 4,
            transient_kind: TransientKind::Slash,
            footer_mode: crate::app::FooterMode::SlashActive,
        };
        let approval = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 8,
            transient_kind: TransientKind::Approval,
            footer_mode: crate::app::FooterMode::ApprovalActive,
        };
        assert_eq!(stable_bottom_chrome_height(slash), 3);
        assert_eq!(stable_bottom_chrome_height(approval), 3);
    }

    #[test]
    fn clamp_transient_rows_respects_live_area() {
        let layout = BottomLayout {
            status_rows: 1,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 8,
            transient_kind: TransientKind::Approval,
            footer_mode: crate::app::FooterMode::ApprovalActive,
        };
        assert_eq!(clamp_transient_rows(8, layout), 3);
    }

    #[test]
    fn slash_clipping_keeps_selected_near_center() {
        let lines = (0..8)
            .map(|i| Line::from(format!("line-{i}")))
            .collect::<Vec<_>>();
        let clipped = clip_transient_lines(TransientKind::Slash, &lines, Some(6), 3);
        let rendered = clipped
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "line-5".to_string(),
                "line-6".to_string(),
                "line-7".to_string()
            ]
        );
    }

    #[test]
    fn approval_clipping_keeps_title_and_actions() {
        let lines = vec![
            Line::from("title"),
            Line::from("reason"),
            Line::from("detail"),
            Line::from("approve"),
            Line::from("deny"),
            Line::from("later"),
        ];
        let clipped = clip_transient_lines(TransientKind::Approval, &lines, Some(4), 4);
        let rendered = clipped
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "title".to_string(),
                "approve".to_string(),
                "deny".to_string(),
                "later".to_string()
            ]
        );
    }
}
