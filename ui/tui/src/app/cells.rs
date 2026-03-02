use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::history_cell::{CellRole, HistoryCell};

use super::App;
use super::tool_output::summarize_tool_output;

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
        tool_name: &str,
        ok: bool,
        output: &str,
    ) -> HistoryCell {
        let mut lines = Vec::new();
        let status = if ok {
            format!("{} completed", tool_name)
        } else {
            format!("{} failed", tool_name)
        };
        lines.push(Line::from(vec![
            Span::styled(
                "• ",
                Style::default()
                    .fg(if ok { Color::Green } else { Color::Red })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(status),
        ]));

        for (idx, preview) in summarize_tool_output(output).into_iter().enumerate() {
            let prefix = if idx == 0 { "  └ " } else { "    " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::Gray)),
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
