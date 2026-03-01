# System Overview

## Purpose
Describe the runtime architecture boundaries in `tui_next`.

## Scope
`terminal/*`, `insert_history`, `tui` orchestrator, and app/state adapter.

## Decisions (Locked)
- `terminal/core` is the single source of viewport/cursor truth.
- `insert_history` is the only history scrollback insertion path.
- `tui` draw lifecycle order is fixed: viewport update -> history insert -> live draw -> cursor sync.
- `app` handles event-to-cell mapping only; it does not own terminal scrolling policy.

## Open Questions
- None for architecture baseline.

## Validation
- Verify multi-turn no-duplicate history and no footer pollution in inline mode.

## Last Updated
2026-03-01
