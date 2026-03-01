# tui Docs

## Purpose
Track architecture, validation, and migration status for the `ui/tui` runtime.

## Scope
This index covers documents under `ui/tui/docs` only.

## Decisions (Locked)
- `ui/tui` is the Codex-core migration runtime.
- Terminal/history behavior must be validated before default switch.

## Validation
- `zsh -lc "cargo build -p tui_next"`
- `zsh -lc "cargo test -p tui_next"`

## Last Updated
2026-03-01

## Document Tree
- `architecture/system-overview.md` (Active)
- `architecture/inline-runtime-notes.md` (Active)
- `testing/gates.md` (Active)
- `progress/status.md` (Active)
