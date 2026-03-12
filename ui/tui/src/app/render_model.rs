use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use std::time::Duration;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::state::ApprovalSelection;
use crate::status::indicator;

use super::{App, FooterMode, TransientKind};

#[derive(Debug, Clone)]
pub struct TransientPanel {
    pub kind: TransientKind,
    pub lines: Vec<Line<'static>>,
    pub selected_index: Option<usize>,
}

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
        let cursor = self.state.input_cursor.min(self.state.input.len());
        let prefix = &self.state.input[..cursor];
        let width = UnicodeWidthStr::width(prefix) as u16;
        let raw = 2u16.saturating_add(width);
        raw.min(area_width.saturating_sub(1))
    }

    pub fn footer_text(&self) -> String {
        match self.bottom_layout(0).footer_mode {
            FooterMode::Idle => "Enter submit | / commands | Esc clear | Ctrl-C quit".to_string(),
            FooterMode::SlashActive => "Tab/Enter complete | Esc dismiss".to_string(),
            FooterMode::ApprovalActive => "↑↓ select | Enter confirm | Esc later".to_string(),
        }
    }

    pub fn slash_palette_lines(&self) -> Option<Vec<Line<'static>>> {
        if !self.state.slash_palette.visible {
            return None;
        }

        if self.state.slash_palette.matches.is_empty() {
            return Some(vec![Line::from(Span::styled(
                "No matching commands",
                Style::default().fg(Color::DarkGray),
            ))]);
        }

        let mut lines = Vec::new();
        for (index, matched) in self.state.slash_palette.matches.iter().enumerate() {
            let selected = index == self.state.slash_palette.selected_index;
            lines.push(Line::from(vec![
                Span::styled(
                    format!("/{:<10}", matched.command_name),
                    Style::default()
                        .fg(if selected { Color::Cyan } else { Color::White })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(matched.description, Style::default().fg(Color::Gray)),
            ]));
        }
        Some(lines)
    }

    pub fn slash_palette_height(&self) -> u16 {
        self.slash_palette_lines()
            .map(|lines| lines.len() as u16)
            .unwrap_or(0)
    }

    pub fn status_bar_line(
        &self,
        now: Instant,
        width: u16,
        animations_enabled: bool,
    ) -> Option<Line<'static>> {
        let status = self.state.status_bar.as_ref()?;
        Some(indicator::status_line(
            status,
            now,
            width,
            animations_enabled,
        ))
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
        let is_shell = matches!(pending.tool_name.as_deref(), Some("shell" | "exec_command"));
        if is_shell {
            return Some(self.shell_approval_panel_lines(pending));
        }

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
        if let Some(command) = &pending.command_preview {
            lines.push(Line::from(Span::styled(
                format!("cmd: {command}"),
                Style::default().fg(Color::Gray),
            )));
        }
        if !pending.risk_tags.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("risks: {}", pending.risk_tags.join(",")),
                Style::default().fg(Color::LightRed),
            )));
        }
        if let Some(backend) = &pending.sandbox_backend {
            let degrade = pending
                .degrade_reason
                .clone()
                .unwrap_or_else(|| "none".to_string());
            lines.push(Line::from(Span::styled(
                format!("sandbox: {backend} degrade: {degrade}"),
                Style::default().fg(Color::DarkGray),
            )));
        }
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
        self.approval_panel_lines()
            .map(|lines| lines.len() as u16)
            .unwrap_or(0)
    }

    pub fn transient_panel(&self) -> Option<TransientPanel> {
        let layout = self.bottom_layout(0);
        match layout.transient_kind {
            TransientKind::Approval => self.approval_panel_lines().map(|lines| {
                let selected = self.approval_selected_line_index(lines.len());
                TransientPanel {
                    kind: TransientKind::Approval,
                    lines,
                    selected_index: selected,
                }
            }),
            TransientKind::Slash => self.slash_palette_lines().map(|lines| {
                let selected = if lines.is_empty() {
                    None
                } else {
                    Some(
                        self.state
                            .slash_palette
                            .selected_index
                            .min(lines.len().saturating_sub(1)),
                    )
                };
                TransientPanel {
                    kind: TransientKind::Slash,
                    lines,
                    selected_index: selected,
                }
            }),
            TransientKind::None => None,
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

    fn shell_approval_panel_lines(
        &self,
        pending: &crate::state::PendingApproval,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            format!(
                "Approval Required ({} remaining)",
                self.approval_remaining_text(pending)
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::default());

        let command = pending
            .command_preview
            .clone()
            .unwrap_or_else(|| pending.target.clone());
        lines.push(Line::from(Span::styled(
            format!("Command: {}", truncate_one_line(&command, 96)),
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::styled(
            "Reason: Sandbox denied execution; fallback needs approval".to_string(),
            Style::default().fg(Color::DarkGray),
        )));
        let detail = format!(
            "{} ({})",
            pending
                .sandbox_backend
                .clone()
                .unwrap_or_else(|| "sandbox".to_string()),
            pending
                .degrade_reason
                .clone()
                .unwrap_or_else(|| pending.reason.clone())
        );
        lines.push(Line::from(Span::styled(
            format!("Detail: {}", truncate_one_line(&detail, 96)),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::default());

        lines.push(self.approval_option_line(
            "Approve and run without sandbox",
            self.state.approval_selection == ApprovalSelection::Approve,
        ));
        lines.push(self.approval_option_line(
            "Deny this request",
            self.state.approval_selection == ApprovalSelection::Deny,
        ));
        lines.push(self.approval_option_line(
            "Decide later",
            self.state.approval_selection == ApprovalSelection::Later,
        ));
        lines
    }

    fn approval_selected_line_index(&self, total_lines: usize) -> Option<usize> {
        if total_lines == 0 {
            return None;
        }
        let action_base = total_lines.saturating_sub(3);
        let offset = match self.state.approval_selection {
            ApprovalSelection::Approve => 0usize,
            ApprovalSelection::Deny => 1usize,
            ApprovalSelection::Later => 2usize,
        };
        Some((action_base + offset).min(total_lines.saturating_sub(1)))
    }

    fn approval_remaining_text(&self, pending: &crate::state::PendingApproval) -> String {
        let elapsed = pending.requested_at.elapsed();
        let timeout = Duration::from_millis(pending.timeout_ms);
        let remaining = timeout.saturating_sub(elapsed);
        let secs = remaining.as_secs();
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

fn truncate_one_line(text: &str, max_chars: usize) -> String {
    let single_line = text.replace(['\n', '\r'], " ");
    let total = single_line.chars().count();
    if total <= max_chars {
        return single_line;
    }
    let mut out = single_line.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}
