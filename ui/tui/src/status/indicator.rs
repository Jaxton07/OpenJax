use std::time::Instant;

use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use super::shimmer::PaletteHint;
use super::shimmer::ShimmerProfile;
use super::shimmer::shimmer_spans;
use crate::state::StatusBarState;

pub(crate) fn status_line(
    status: &StatusBarState,
    now: Instant,
    _width: u16,
    animations_enabled: bool,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(8);
    spans.push("• ".dim());
    if animations_enabled {
        spans.extend(shimmer_spans(
            &status.label,
            now,
            status.started_at,
            &ShimmerProfile::default(),
            PaletteHint::detect(),
        ));
    } else {
        spans.push(Span::raw(status.label.clone()));
    }

    let elapsed_secs = now.saturating_duration_since(status.started_at).as_secs();
    if status.show_interrupt_hint {
        spans.push(format!(" ({} • ", fmt_elapsed_compact(elapsed_secs)).dim());
        spans.push("esc".bold().underlined());
        spans.push(" to interrupt)".dim());
    } else {
        spans.push(format!(" ({})", fmt_elapsed_compact(elapsed_secs)).dim());
    }
    Line::from(spans)
}

pub(crate) fn fmt_elapsed_compact(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        return format!("{elapsed_secs}s");
    }
    if elapsed_secs < 3_600 {
        let minutes = elapsed_secs / 60;
        let seconds = elapsed_secs % 60;
        return format!("{minutes}m {seconds:02}s");
    }
    let hours = elapsed_secs / 3_600;
    let minutes = (elapsed_secs % 3_600) / 60;
    let seconds = elapsed_secs % 60;
    format!("{hours}h {minutes:02}m {seconds:02}s")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::state::StatusPhase;

    #[test]
    fn line_contains_working_elapsed_and_interrupt_hint() {
        let started = Instant::now() - Duration::from_secs(3);
        let status = StatusBarState {
            phase: StatusPhase::Running,
            label: "Working".to_string(),
            show_interrupt_hint: true,
            started_at: started,
        };

        let line = status_line(&status, Instant::now(), 120, false);
        let text = line.to_string();
        assert!(text.contains("Working"));
        assert!(text.contains("interrupt"));
        assert!(text.contains("s"));
    }

    #[test]
    fn no_animation_keeps_plain_text_label() {
        let started = Instant::now() - Duration::from_secs(1);
        let status = StatusBarState {
            phase: StatusPhase::Running,
            label: "Working".to_string(),
            show_interrupt_hint: true,
            started_at: started,
        };
        let line = status_line(&status, Instant::now(), 120, false);
        assert!(line.to_string().contains("Working"));
    }
}
