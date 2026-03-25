//! Viewport planning, layout constraint building, and transient-panel line clipping.
//!
//! All functions here are pure (no I/O, no terminal state) and operate solely on
//! `BottomLayout`, `Rect`, and `Line` values.  They are used by `tui.rs` to compute
//! draw geometry before issuing terminal commands.

use std::collections::BTreeSet;

use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;

use crate::app::{BottomLayout, TransientKind};

// ---------------------------------------------------------------------------
// Viewport planning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ViewportPlan {
    pub(crate) area: Rect,
    pub(crate) scroll_up: u16,
}

pub(crate) fn compute_viewport_plan(
    current_area: Rect,
    screen_width: u16,
    screen_height: u16,
    desired_height: u16,
) -> ViewportPlan {
    let bounded_height = desired_height.clamp(8, screen_height.max(8));
    let mut area = current_area;
    area.width = screen_width;
    area.height = bounded_height.min(screen_height);

    let mut scroll_up = 0;
    if area.bottom() > screen_height {
        scroll_up = area.bottom().saturating_sub(screen_height);
        area.y = screen_height.saturating_sub(area.height);
    }

    ViewportPlan { area, scroll_up }
}

pub(crate) fn stable_bottom_chrome_height(layout: BottomLayout) -> u16 {
    layout
        .status_rows
        .saturating_add(layout.input_rows)
        .saturating_add(layout.footer_rows)
}

// ---------------------------------------------------------------------------
// Layout constraints
// ---------------------------------------------------------------------------

pub(crate) fn clamp_transient_rows(total_height: u16, layout: BottomLayout) -> u16 {
    if layout.transient_rows == 0 {
        return 0;
    }

    let fixed_rows = layout
        .status_rows
        .saturating_add(layout.input_rows)
        .saturating_add(layout.footer_rows);
    let max_transient = total_height.saturating_sub(fixed_rows.saturating_add(1));
    layout.transient_rows.min(max_transient)
}

pub(crate) fn layout_constraints(layout: BottomLayout, transient_rows: u16) -> Vec<Constraint> {
    let mut constraints = vec![Constraint::Min(1)];
    if layout.status_rows > 0 {
        constraints.push(Constraint::Length(layout.status_rows));
    }
    if transient_rows > 0 {
        constraints.push(Constraint::Length(transient_rows));
    }
    constraints.push(Constraint::Length(layout.input_rows));
    constraints.push(Constraint::Length(layout.footer_rows));
    constraints
}

// ---------------------------------------------------------------------------
// Named chunk indices
// ---------------------------------------------------------------------------

/// Named chunk indices for the vertical layout produced by `layout_constraints`.
/// Avoids manual counter increments that are easy to mis-count when zones are added.
pub(crate) struct ChunkIndices {
    pub(crate) live: usize,
    pub(crate) status: Option<usize>,
    pub(crate) transient: Option<usize>,
    pub(crate) input: usize,
    pub(crate) footer: usize,
}

pub(crate) fn build_chunk_indices(layout: BottomLayout, transient_rows: u16) -> ChunkIndices {
    let mut next = 1usize;
    let status = (layout.status_rows > 0).then(|| {
        let i = next;
        next += 1;
        i
    });
    let transient = (transient_rows > 0).then(|| {
        let i = next;
        next += 1;
        i
    });
    let input = next;
    next += 1;
    let footer = next;
    ChunkIndices {
        live: 0,
        status,
        transient,
        input,
        footer,
    }
}

// ---------------------------------------------------------------------------
// Transient-panel line clipping
// ---------------------------------------------------------------------------

pub(crate) fn clip_transient_lines(
    kind: TransientKind,
    lines: &[Line<'static>],
    selected_index: Option<usize>,
    max_rows: usize,
) -> Vec<Line<'static>> {
    if max_rows == 0 || lines.is_empty() {
        return Vec::new();
    }
    if lines.len() <= max_rows {
        return lines.to_vec();
    }

    match kind {
        TransientKind::Slash => {
            let selected = selected_index
                .unwrap_or(0)
                .min(lines.len().saturating_sub(1));
            let start = selected
                .saturating_sub(max_rows / 2)
                .min(lines.len().saturating_sub(max_rows));
            lines[start..start + max_rows].to_vec()
        }
        TransientKind::Approval => clip_approval_lines(lines, selected_index, max_rows),
        TransientKind::PolicyPicker => clip_approval_lines(lines, selected_index, max_rows),
        TransientKind::None => Vec::new(),
    }
}

fn clip_approval_lines(
    lines: &[Line<'static>],
    selected_index: Option<usize>,
    max_rows: usize,
) -> Vec<Line<'static>> {
    if max_rows == 0 || lines.is_empty() {
        return Vec::new();
    }

    let mut chosen: BTreeSet<usize> = BTreeSet::new();
    chosen.insert(0);

    if max_rows > 1 {
        if let Some(idx) = selected_index {
            chosen.insert(idx.min(lines.len().saturating_sub(1)));
        }
        let action_start = lines.len().saturating_sub(3);
        for idx in action_start..lines.len() {
            if chosen.len() >= max_rows {
                break;
            }
            chosen.insert(idx);
        }
        for idx in 1..action_start {
            if chosen.len() >= max_rows {
                break;
            }
            chosen.insert(idx);
        }
    }

    chosen
        .into_iter()
        .take(max_rows)
        .map(|idx| lines[idx].clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use ratatui::layout::Constraint;
    use ratatui::layout::Rect;
    use ratatui::text::Line;

    use crate::app::{BottomLayout, FooterMode, TransientKind};

    use super::{
        ChunkIndices, ViewportPlan, build_chunk_indices, clamp_transient_rows,
        clip_transient_lines, compute_viewport_plan, layout_constraints,
        stable_bottom_chrome_height,
    };

    #[test]
    fn chunk_indices_all_zones_present() {
        let layout = BottomLayout {
            status_rows: 1,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 3,
            transient_kind: TransientKind::Slash,
            footer_mode: FooterMode::SlashActive,
        };
        // live=0, status=1, transient=2, input=3, footer=4
        let idx: ChunkIndices = build_chunk_indices(layout, 3);
        assert_eq!(idx.live, 0);
        assert_eq!(idx.status, Some(1));
        assert_eq!(idx.transient, Some(2));
        assert_eq!(idx.input, 3);
        assert_eq!(idx.footer, 4);
    }

    #[test]
    fn chunk_indices_no_optional_zones() {
        let layout = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 0,
            transient_kind: TransientKind::None,
            footer_mode: FooterMode::Idle,
        };
        // live=0, input=1, footer=2
        let idx: ChunkIndices = build_chunk_indices(layout, 0);
        assert_eq!(idx.live, 0);
        assert_eq!(idx.status, None);
        assert_eq!(idx.transient, None);
        assert_eq!(idx.input, 1);
        assert_eq!(idx.footer, 2);
    }

    #[test]
    fn viewport_stays_near_current_cursor_when_space_is_available() {
        let current = Rect::new(0, 10, 0, 0);
        let next = compute_viewport_plan(current, 100, 40, 12);
        assert_eq!(
            next,
            ViewportPlan {
                area: Rect::new(0, 10, 100, 12),
                scroll_up: 0,
            }
        );
    }

    #[test]
    fn viewport_overflow_requests_scroll_and_clamps_to_bottom() {
        let current = Rect::new(0, 25, 0, 0);
        let next = compute_viewport_plan(current, 100, 30, 12);
        assert_eq!(
            next,
            ViewportPlan {
                area: Rect::new(0, 18, 100, 12),
                scroll_up: 7,
            }
        );
    }

    #[test]
    fn layout_places_status_row_above_input() {
        let layout = BottomLayout {
            status_rows: 1,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 0,
            transient_kind: TransientKind::None,
            footer_mode: FooterMode::Idle,
        };
        let constraints = layout_constraints(layout, 0);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn layout_places_slash_palette_below_input() {
        let layout = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 3,
            transient_kind: TransientKind::Slash,
            footer_mode: FooterMode::SlashActive,
        };
        let constraints = layout_constraints(layout, 3);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn layout_places_transient_region_between_input_and_footer() {
        let layout = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 5,
            transient_kind: TransientKind::Slash,
            footer_mode: FooterMode::SlashActive,
        };
        let constraints = layout_constraints(layout, 5);
        assert_eq!(
            constraints,
            vec![
                Constraint::Min(1),
                Constraint::Length(5),
                Constraint::Length(2),
                Constraint::Length(1),
            ]
        );
    }

    #[test]
    fn stable_bottom_chrome_is_not_affected_by_transient_panels() {
        let slash = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 4,
            transient_kind: TransientKind::Slash,
            footer_mode: FooterMode::SlashActive,
        };
        let approval = BottomLayout {
            status_rows: 0,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 8,
            transient_kind: TransientKind::Approval,
            footer_mode: FooterMode::ApprovalActive,
        };
        assert_eq!(stable_bottom_chrome_height(slash), 3);
        assert_eq!(stable_bottom_chrome_height(approval), 3);
    }

    #[test]
    fn clamp_transient_rows_respects_live_area() {
        let layout = BottomLayout {
            status_rows: 1,
            input_rows: 2,
            footer_rows: 1,
            transient_rows: 8,
            transient_kind: TransientKind::Approval,
            footer_mode: FooterMode::ApprovalActive,
        };
        assert_eq!(clamp_transient_rows(8, layout), 3);
    }

    #[test]
    fn slash_clipping_keeps_selected_near_center() {
        let lines = (0..8)
            .map(|i| Line::from(format!("line-{i}")))
            .collect::<Vec<_>>();
        let clipped = clip_transient_lines(TransientKind::Slash, &lines, Some(6), 3);
        let rendered = clipped
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "line-5".to_string(),
                "line-6".to_string(),
                "line-7".to_string()
            ]
        );
    }

    #[test]
    fn approval_clipping_keeps_title_and_actions() {
        let lines = vec![
            Line::from("title"),
            Line::from("reason"),
            Line::from("detail"),
            Line::from("approve"),
            Line::from("deny"),
            Line::from("later"),
        ];
        let clipped = clip_transient_lines(TransientKind::Approval, &lines, Some(4), 4);
        let rendered = clipped
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "title".to_string(),
                "approve".to_string(),
                "deny".to_string(),
                "later".to_string()
            ]
        );
    }
}
