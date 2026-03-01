# tui_next Docs

## Purpose
Track architecture, validation, and migration status for the `ui/tui_next` runtime.

## Scope
This index covers documents under `ui/tui_next/docs` only.

## Decisions (Locked)
- `tui_next` is the Codex-core migration runtime.
- `ui/tui` has been removed; `tui_next` is the single maintained runtime.
- Terminal/history behavior must be validated before default switch.

## Open Questions
- Whether to rename package `tui_next` to `tui` in a follow-up.

## Validation
- `zsh -lc "cargo build -p tui_next"`
- `zsh -lc "cargo test -p tui_next"`

## Last Updated
2026-03-01

## Document Tree
- `architecture/system-overview.md` (Active)
- `testing/gates.md` (Active)
- `progress/status.md` (Active)
