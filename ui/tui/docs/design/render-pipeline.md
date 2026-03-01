# Render Pipeline

## Purpose
Describe how the runtime transforms app state into terminal output.

## Scope
Live area rendering, input/footer rendering, and history insertion boundaries.

## Decisions (Locked)
- Live area and history area are separate responsibilities.
- Input/footer are rendered in fixed bottom rows inside viewport.

## Open Questions
- Overlay stacking order for approvals/help panes.

## Validation
- Snapshot tests should verify stable render layout with and without pending history.

## Last Updated
2026-03-01
