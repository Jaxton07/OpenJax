# Runtime Lifecycle

## Purpose
Specify the per-frame draw and insertion lifecycle.

## Scope
`ui/tui/src/tui.rs`, `custom_terminal.rs`, `insert_history.rs`.

## Decisions (Locked)
- Frame lifecycle order: update viewport -> insert pending history -> draw live -> sync cursor.
- Pending history is consumed exactly once per draw cycle.

## Open Questions
- None.

## Validation
- `Tui::draw` executes the fixed order.

## Last Updated
2026-03-01
