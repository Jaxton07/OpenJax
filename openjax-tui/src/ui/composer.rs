use ratatui::text::Line;

use crate::state::AppState;

pub fn render_line(state: &AppState) -> Line<'static> {
    let (text, _) = composer_view(state, usize::MAX);
    Line::from(text)
}

pub fn cursor_offset(state: &AppState, inner_width: u16) -> u16 {
    let (_, offset) = composer_view(state, inner_width as usize);
    offset
}

fn composer_view(state: &AppState, inner_width: usize) -> (String, u16) {
    let prompt = "> ";
    let prompt_len = prompt.chars().count();
    if inner_width <= prompt_len {
        return (prompt.to_string(), 0);
    }

    let chars: Vec<char> = state.input.chars().collect();
    let cursor = state.input_cursor.min(chars.len());
    let content_width = inner_width - prompt_len;

    let mut start = 0usize;
    if chars.len() > content_width {
        start = cursor.saturating_sub(content_width.saturating_sub(1));
        if start + content_width > chars.len() {
            start = chars.len().saturating_sub(content_width);
        }
    }

    let end = (start + content_width).min(chars.len());
    let visible: String = chars[start..end].iter().collect();
    let cursor_in_visible = cursor.saturating_sub(start).min(content_width);
    let cursor_col = (prompt_len + cursor_in_visible) as u16;

    (format!("{prompt}{visible}"), cursor_col)
}
