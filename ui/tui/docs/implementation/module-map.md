# Module Map

## Purpose
Map source files to responsibilities.

## Scope
All files under `ui/tui/src`.

## Decisions (Locked)
- `app.rs`: state transitions and content partition.
- `tui.rs`: draw orchestration and queue draining.
- `custom_terminal.rs`: terminal viewport and cursor state.
- `insert_history.rs`: scrollback insertion implementation.
- `input.rs`: event mapping.

## Open Questions
- When to split `app.rs` into smaller modules.

## Validation
- Every module listed has a single primary responsibility.

## Last Updated
2026-03-01
