use ratatui::text::Line;

use crate::state::{AppState, ApprovalSelection};

pub fn render_lines(state: &AppState) -> Vec<Line<'static>> {
    let Some(overlay) = state.approval.overlay.as_ref() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    lines.push(Line::from(overlay.summary.clone()));
    let options = ["Approve", "Deny", "Cancel or decide later"];
    for (idx, label) in options.iter().enumerate() {
        let prefix = if idx == overlay.selected_index {
            "› "
        } else {
            "  "
        };
        lines.push(Line::from(format!("{prefix}{label}")));
    }
    lines
}

pub fn confirm_selection(state: &AppState) -> Option<ApprovalSelection> {
    state.approval.selection()
}
