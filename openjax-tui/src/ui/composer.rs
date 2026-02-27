use ratatui::text::Line;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    let prompt_width = prompt.width();
    if inner_width <= prompt_width {
        return (prompt.to_string(), 0);
    }

    let chars: Vec<char> = state.input_state.buffer.chars().collect();
    let cursor = state.input_state.cursor.min(chars.len());
    let content_width = inner_width - prompt_width;

    let mut start = 0usize;
    while display_width(&chars[start..cursor]) > content_width.saturating_sub(1) {
        start += 1;
    }

    let mut end = start;
    let mut visible_width = 0usize;
    while end < chars.len() {
        let ch_width = char_width(chars[end]);
        if visible_width + ch_width > content_width {
            break;
        }
        visible_width += ch_width;
        end += 1;
    }

    while cursor > end {
        start += 1;
        while display_width(&chars[start..cursor]) > content_width.saturating_sub(1) {
            start += 1;
        }
        end = start;
        visible_width = 0;
        while end < chars.len() {
            let ch_width = char_width(chars[end]);
            if visible_width + ch_width > content_width {
                break;
            }
            visible_width += ch_width;
            end += 1;
        }
    }

    let visible: String = chars[start..end].iter().collect();
    let cursor_col = (prompt_width + display_width(&chars[start..cursor])) as u16;

    (format!("{prompt}{visible}"), cursor_col)
}

fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

fn display_width(chars: &[char]) -> usize {
    chars.iter().map(|ch| char_width(*ch)).sum()
}

#[cfg(test)]
mod tests {
    use super::cursor_offset;
    use crate::state::AppState;

    #[test]
    fn cursor_offset_respects_wide_chars() {
        let mut state = AppState::with_defaults();
        state.input_state.buffer = "你好".to_string();
        state.input_state.cursor = state.input_state.buffer.chars().count();
        let offset = cursor_offset(&state, 20);
        assert_eq!(offset, 6);
    }
}
