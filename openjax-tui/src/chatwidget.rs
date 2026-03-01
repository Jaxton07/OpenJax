use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::state::{AppState, RenderKind, UiMessage};

#[derive(Debug, Default)]
pub struct ChatWidget;

impl ChatWidget {
    pub fn render_lines(state: &AppState) -> Vec<Line<'static>> {
        Self::render_message_lines(&state.transcript.messages)
    }

    pub fn render_message_lines(messages: &[UiMessage]) -> Vec<Line<'static>> {
        let mut out = Vec::new();

        for message in messages {
            match message.role.as_str() {
                "user" => {
                    out.push(Line::from(vec![
                        Span::styled("❯ ", Style::default().fg(Color::Cyan)),
                        Span::raw(message.content.clone()),
                    ]));
                }
                "assistant" => {
                    let prefix = Span::styled("⏺ ", Style::default().fg(Color::Green));
                    if message.render_kind == RenderKind::Markdown {
                        let mut first = true;
                        for ln in message.content.lines() {
                            if first {
                                out.push(Line::from(vec![
                                    prefix.clone(),
                                    Span::raw(ln.to_string()),
                                ]));
                                first = false;
                            } else {
                                out.push(Line::from(Span::raw(ln.to_string())));
                            }
                        }
                        if first {
                            out.push(Line::from(vec![prefix, Span::raw("")]));
                        }
                    } else {
                        out.push(Line::from(vec![prefix, Span::raw(message.content.clone())]));
                    }
                }
                "tool" => {
                    let color = if message.ok { Color::Green } else { Color::Red };
                    let mut text = message.content.clone();
                    if let Some(target) = message.target.as_ref() {
                        text.push_str(&format!(" ({target})"));
                    }
                    out.push(Line::from(vec![
                        Span::styled("⏺ ", Style::default().fg(color)),
                        Span::raw(text),
                    ]));
                }
                "system" => {
                    out.push(Line::from(Span::styled(
                        message.content.clone(),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                _ => {
                    out.push(Line::from(Span::raw(message.content.clone())));
                }
            }
            out.push(Line::from(""));
        }

        out
    }

    pub fn render_live_lines(state: &AppState) -> Vec<Line<'static>> {
        let start = state
            .history_emission
            .emitted_message_count
            .min(state.transcript.messages.len());
        Self::render_message_lines(&state.transcript.messages[start..])
    }

    pub fn desired_height(state: &AppState, width: u16) -> u16 {
        visual_line_count(&Self::render_live_lines(state), width) as u16
    }
}

pub fn visual_line_count(lines: &[Line<'_>], width: u16) -> usize {
    let wrap_width = usize::from(width.max(1));
    let mut total = 0usize;
    for line in lines {
        let text = line.to_string();
        if text.is_empty() {
            total += 1;
            continue;
        }
        total += textwrap::wrap(&text, wrap_width).len().max(1);
    }
    total.max(1)
}
