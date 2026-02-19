use ratatui::text::Line;

use crate::state::AppState;

pub fn render_line(state: &AppState) -> Line<'static> {
    let shortcuts = if state.show_help {
        "Enter submit | Backspace delete | ? hide help | q quit"
    } else {
        "Enter submit | ? help | q quit"
    };

    let runtime = format!(
        "model: {} | approval: {} | sandbox: {}",
        state.model_name.as_deref().unwrap_or("-"),
        state.approval_policy.as_deref().unwrap_or("-"),
        state.sandbox_mode.as_deref().unwrap_or("-")
    );

    Line::from(format!("{shortcuts} || {runtime}"))
}
