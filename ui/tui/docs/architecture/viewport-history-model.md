# Viewport and History Model

## Purpose
Capture viewport and history invariants.

## Scope
History cell queueing and viewport positioning in inline mode.

## Decisions (Locked)
- Viewport area is persisted by terminal core and reused each frame.
- History insertion operates on committed cells only.
- Live rendering excludes already inserted history cells.

## Open Questions
- Alt-screen behavior parity details with future overlays.

## Validation
- Tests must assert no duplicate message appears in history and live areas.

## Last Updated
2026-03-01
