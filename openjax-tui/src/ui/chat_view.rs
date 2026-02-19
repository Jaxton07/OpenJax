use ratatui::text::Line;

use crate::state::AppState;

pub fn render_lines(state: &AppState) -> Vec<Line<'static>> {
    state
        .messages
        .iter()
        .map(|m| Line::from(format!("[{}] {}", m.role, m.content)))
        .collect()
}
