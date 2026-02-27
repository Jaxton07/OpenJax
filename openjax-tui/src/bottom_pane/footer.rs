use ratatui::text::Line;

use crate::state::{AppState, TurnPhase};

pub fn render_line(state: &AppState) -> Line<'static> {
    let left = if state.approval.overlay_visible {
        "Approval pending: Up/Down choose, Enter confirm, Esc defer".to_string()
    } else if state.input_state.slash_popup.open {
        "Slash mode: Up/Down choose, Enter run, Esc close".to_string()
    } else {
        match state.turn.phase {
            TurnPhase::Thinking => "Thinking...".to_string(),
            TurnPhase::Streaming => "Streaming...".to_string(),
            TurnPhase::Error => "Error state".to_string(),
            TurnPhase::Idle => "Enter submit | / commands | Ctrl-C quit".to_string(),
        }
    };
    let right = format!(
        "model={} | approval={} | sandbox={}",
        state.model_name.as_deref().unwrap_or("-"),
        state.approval_policy.as_deref().unwrap_or("-"),
        state.sandbox_mode.as_deref().unwrap_or("-")
    );
    Line::from(format!("{left} || {right}"))
}
