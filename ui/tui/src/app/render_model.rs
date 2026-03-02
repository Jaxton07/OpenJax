use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::state::ApprovalSelection;

use super::App;

impl App {
    pub fn render_live(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let paragraph = Paragraph::new(self.live_chat_lines()).wrap(Wrap { trim: false });
        let total_lines = self.live_visual_height(area.width) as usize;
        let scroll = total_lines.saturating_sub(area.height as usize);
        let paragraph = paragraph.scroll((scroll.min(u16::MAX as usize) as u16, 0));
        paragraph.render(area, buf);
    }

    pub fn live_chat_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if !self.state.history_cells.is_empty() && !self.state.live_messages.is_empty() {
            lines.push(Line::default());
        }

        for (message_idx, message) in self.state.live_messages.iter().enumerate() {
            if message_idx > 0 {
                lines.push(Line::default());
            }

            let mut pushed = false;
            for (idx, raw_line) in message.content.lines().enumerate() {
                let prefix = if idx == 0 {
                    format!("{} ", message.role)
                } else {
                    "  ".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::raw(raw_line.to_string()),
                ]));
                pushed = true;
            }

            if !pushed {
                lines.push(Line::from(Span::styled(
                    format!("{} ...", message.role),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        lines
    }

    pub fn input_line(&self) -> Line<'static> {
        let mut spans = vec![Span::styled(
            "❯ ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        spans.push(Span::raw(self.state.input.clone()));
        if self.state.input.is_empty() {
            spans.push(Span::styled("_", Style::default().fg(Color::DarkGray)));
        }
        Line::from(spans)
    }

    pub fn input_cursor_offset(&self, area_width: u16) -> u16 {
        let width = UnicodeWidthStr::width(self.state.input.as_str()) as u16;
        let raw = 2u16.saturating_add(width);
        raw.min(area_width.saturating_sub(1))
    }

    pub fn footer_text(&self) -> String {
        format!(
            "Enter submit | / commands | Ctrl-C quit || model={} | approval={} | sandbox={}",
            self.state.model_name.as_deref().unwrap_or("unknown"),
            self.state.approval_policy.as_deref().unwrap_or("unknown"),
            self.state.sandbox_mode.as_deref().unwrap_or("unknown"),
        )
    }

    pub fn move_approval_selection(&mut self, delta: i8) {
        if self.state.pending_approval.is_none() {
            return;
        }
        let current = match self.state.approval_selection {
            ApprovalSelection::Approve => 0i8,
            ApprovalSelection::Deny => 1i8,
            ApprovalSelection::Later => 2i8,
        };
        let next = (current + delta).rem_euclid(3);
        self.state.approval_selection = ApprovalSelection::from_index(next as usize);
    }

    pub fn approval_panel_lines(&self) -> Option<Vec<Line<'static>>> {
        let pending = self.state.pending_approval.as_ref()?;
        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            format!("是否允许执行操作: {}", pending.target),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            pending.reason.clone(),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(self.approval_option_line(
            "Approve",
            self.state.approval_selection == ApprovalSelection::Approve,
        ));
        lines.push(self.approval_option_line(
            "Deny",
            self.state.approval_selection == ApprovalSelection::Deny,
        ));
        lines.push(self.approval_option_line(
            "Cancel or decide later",
            self.state.approval_selection == ApprovalSelection::Later,
        ));
        Some(lines)
    }

    pub fn approval_panel_height(&self) -> u16 {
        if self.state.pending_approval.is_some() {
            5
        } else {
            0
        }
    }

    fn approval_option_line(&self, label: &str, selected: bool) -> Line<'static> {
        let marker = if selected { "› " } else { "  " };
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Line::from(vec![
            Span::styled(marker, style),
            Span::styled(label.to_string(), style),
        ])
    }
}
