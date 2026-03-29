use openjax_protocol::ShellExecutionMetadata;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::history_cell::{CellRole, HistoryCell};

use super::App;
use super::tool_output::{
    degraded_risk_summary, extract_backend_summary, is_partial_result, skill_trigger_guard_hint,
    summarize_tool_output,
};

impl App {
    pub(crate) fn user_cell(&mut self, input: &str) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::User,
            committed: true,
            lines: vec![Line::from(vec![
                Span::styled(
                    "❯ ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(input.to_string()),
            ])],
        }
    }

    pub(crate) fn assistant_cell(&mut self, content: &str) -> HistoryCell {
        let mut lines = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            if idx == 0 {
                lines.push(Line::from(vec![
                    Span::styled(
                        "⏺ ",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(line.to_string()),
                ]));
            } else {
                lines.push(Line::from(format!("  {}", line)));
            }
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "⏺",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Assistant,
            committed: true,
            lines,
        }
    }

    pub(crate) fn tool_cell(&mut self, content: String) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Tool,
            committed: true,
            lines: vec![Line::from(vec![
                Span::styled(
                    "• ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(content),
            ])],
        }
    }

    pub(crate) fn tool_completed_cell(
        &mut self,
        display_name: &str,
        ok: bool,
        output: &str,
        shell_metadata: Option<&ShellExecutionMetadata>,
    ) -> HistoryCell {
        let mut lines = Vec::new();
        let is_partial = is_partial_result(shell_metadata, output);
        let (status, dot_color) = if is_partial {
            (format!("{} partial", display_name), Color::Yellow)
        } else if ok {
            (format!("{} completed", display_name), Color::Green)
        } else {
            (format!("{} failed", display_name), Color::Red)
        };
        lines.push(Line::from(vec![
            Span::styled(
                "• ",
                Style::default().fg(dot_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(status),
        ]));
        if let Some(backend) = extract_backend_summary(shell_metadata, output) {
            lines.push(Line::from(vec![
                Span::styled("  ├ ", Style::default().fg(Color::DarkGray)),
                Span::styled(backend, Style::default().fg(Color::LightBlue)),
            ]));
        }
        if let Some(risk) = degraded_risk_summary(shell_metadata, output) {
            lines.push(Line::from(vec![
                Span::styled("  ├ ", Style::default().fg(Color::DarkGray)),
                Span::styled(risk, Style::default().fg(Color::LightRed)),
            ]));
        }
        if let Some(hint) = skill_trigger_guard_hint(shell_metadata, output) {
            lines.push(Line::from(vec![
                Span::styled("  ├ ", Style::default().fg(Color::DarkGray)),
                Span::styled(hint, Style::default().fg(Color::Yellow)),
            ]));
        }

        for (idx, preview) in summarize_tool_output(output).into_iter().enumerate() {
            let prefix = if idx == 0 { "  └ " } else { "    " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    preview,
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
            ]));
        }

        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::Tool,
            committed: true,
            lines,
        }
    }

    pub(crate) fn system_cell(&mut self, content: String) -> HistoryCell {
        HistoryCell {
            id: self.alloc_id(),
            role: CellRole::System,
            committed: true,
            lines: vec![Line::from(vec![Span::styled(
                content,
                Style::default().fg(Color::DarkGray),
            )])],
        }
    }
}
