use ratatui::text::{Line, Span};

use crate::render::theme;
use crate::state::AppState;

pub fn render_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut out = Vec::new();

    for message in &state.messages {
        let prefix = format!("[{}] ", message.role);
        let padding = " ".repeat(prefix.chars().count());
        let mut content_lines = message.content.lines();
        let first = content_lines.next().unwrap_or_default();

        out.push(Line::from(vec![
            Span::styled(prefix.clone(), theme::role_style(&message.role)),
            Span::raw(first.to_string()),
        ]));

        for line in content_lines {
            out.push(Line::from(vec![
                Span::raw(padding.clone()),
                Span::raw(line.to_string()),
            ]));
        }
    }

    out
}
