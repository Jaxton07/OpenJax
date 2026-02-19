use ratatui::text::Line;

use crate::state::AppState;

pub fn render_line(state: &AppState) -> Line<'static> {
    Line::from(format!("> {}", state.input))
}
