use ratatui::text::Line;
use unicode_width::UnicodeWidthChar;

use super::App;

impl App {
    pub fn desired_height(&self, width: u16) -> u16 {
        let footer_h = 1u16;
        let input_h = 2u16;
        let status_h = if self.state.status_bar.is_some() {
            1u16
        } else {
            0u16
        };
        let approval_h = if self.state.pending_approval.is_some() {
            self.approval_panel_height()
        } else {
            0
        };
        let approval_spacing = if approval_h > 0 { 2u16 } else { 0u16 };
        self.live_visual_height(width)
            .saturating_add(status_h)
            .saturating_add(input_h)
            .saturating_add(approval_h)
            .saturating_add(approval_spacing)
            .saturating_add(footer_h)
            .max(8)
    }

    pub(crate) fn live_visual_height(&self, width: u16) -> u16 {
        if self.state.live_messages.is_empty() {
            return 1;
        }
        let max_w = width.max(8) as usize;
        let mut total = 0usize;
        for line in self.live_chat_lines() {
            total += visual_line_count(&line, max_w);
        }
        total.max(1) as u16
    }
}

fn visual_line_count(line: &Line<'static>, max_w: usize) -> usize {
    if max_w == 0 {
        return 1;
    }
    let mut lines = 1usize;
    let mut current = 0usize;

    for span in &line.spans {
        for ch in span.content.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
            if current + ch_w > max_w {
                lines += 1;
                current = 0;
            }
            current += ch_w;
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use crate::app::App;

    #[test]
    fn desired_height_includes_status_row_when_visible() {
        let mut app = App::default();
        let base = app.desired_height(80);
        app.set_status_running("Working");
        let with_status = app.desired_height(80);
        assert!(with_status >= base);
        assert!(with_status.saturating_sub(base) <= 1);
    }
}
