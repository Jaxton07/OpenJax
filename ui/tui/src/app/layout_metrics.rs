use ratatui::text::Line;
use unicode_width::UnicodeWidthChar;

use super::App;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TransientKind {
    None,
    Slash,
    Approval,
    PolicyPicker,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FooterMode {
    Idle,
    SlashActive,
    ApprovalActive,
    PolicyPickerActive,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BottomLayout {
    pub status_rows: u16,
    pub input_rows: u16,
    pub footer_rows: u16,
    pub transient_rows: u16,
    pub transient_kind: TransientKind,
    pub footer_mode: FooterMode,
}

impl App {
    pub fn bottom_layout(
        &self,
        _width: u16, /* reserved: future width-aware input wrapping */
    ) -> BottomLayout {
        let approval_rows = if self.state.pending_approval.is_some() {
            self.approval_panel_height()
        } else {
            0
        };
        let slash_rows = if approval_rows > 0 {
            0
        } else {
            self.slash_palette_height()
        };
        let picker_rows = if approval_rows > 0 || slash_rows > 0 {
            0
        } else {
            self.policy_picker_height()
        };

        let (transient_kind, transient_rows, footer_mode) = if approval_rows > 0 {
            (
                TransientKind::Approval,
                approval_rows,
                FooterMode::ApprovalActive,
            )
        } else if picker_rows > 0 {
            (
                TransientKind::PolicyPicker,
                picker_rows,
                FooterMode::PolicyPickerActive,
            )
        } else if slash_rows > 0 {
            (TransientKind::Slash, slash_rows, FooterMode::SlashActive)
        } else {
            (TransientKind::None, 0, FooterMode::Idle)
        };

        BottomLayout {
            status_rows: if self.state.status_bar.is_some() {
                1
            } else {
                0
            },
            input_rows: 2,
            footer_rows: 1,
            transient_rows,
            transient_kind,
            footer_mode,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        let bottom = self.bottom_layout(width);
        self.live_visual_height(width)
            .saturating_add(bottom.status_rows)
            .saturating_add(bottom.input_rows)
            .saturating_add(bottom.footer_rows)
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
    use crate::app::{App, FooterMode, TransientKind};

    #[test]
    fn desired_height_includes_status_row_when_visible() {
        let mut app = App::default();
        let base = app.desired_height(80);
        app.set_status_running("Working");
        let with_status = app.desired_height(80);
        assert!(with_status >= base);
        assert!(with_status.saturating_sub(base) <= 1);
    }

    #[test]
    fn slash_palette_height_includes_border_for_single_match() {
        let mut app = App::default();
        // Use /cle to match only 'clear' (cls doesn't start with 'cle')
        app.append_input("/cle");

        assert!(app.is_slash_palette_active());
        assert_eq!(app.state.slash_palette.matches.len(), 1);
        assert_eq!(app.state.slash_palette.matches[0].command_name, "clear");
        assert_eq!(app.slash_palette_height(), 1);
    }

    #[test]
    fn slash_palette_height_includes_border_for_empty_state() {
        let mut app = App::default();
        app.append_input("/z");

        assert!(app.is_slash_palette_active());
        assert!(app.state.slash_palette.matches.is_empty());
        assert_eq!(app.slash_palette_height(), 1);
    }

    #[test]
    fn desired_height_does_not_change_when_slash_palette_opens() {
        let mut app = App::default();
        let base = app.desired_height(80);
        app.append_input("/");
        assert!(app.is_slash_palette_active());
        let with_slash = app.desired_height(80);
        assert_eq!(with_slash, base);
    }

    #[test]
    fn bottom_layout_keeps_footer_row_when_transient_is_visible() {
        let mut app = App::default();
        app.append_input("/");
        let slash_layout = app.bottom_layout(80);
        assert_eq!(slash_layout.footer_rows, 1);
        assert_eq!(slash_layout.transient_kind, TransientKind::Slash);
        assert_eq!(slash_layout.footer_mode, FooterMode::SlashActive);
    }
}
