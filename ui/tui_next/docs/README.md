# tui_next Docs

## Purpose
Track architecture, validation, and migration status for the `ui/tui_next` runtime.

## Scope
This index covers documents under `ui/tui_next/docs` only.

## Decisions (Locked)
- `tui_next` is the Codex-core migration runtime.
- Dual-track rollout is controlled by `OPENJAX_TUI_RUNTIME`.
- Terminal/history behavior must be validated before default switch.

## Open Questions
- Default switch date for `OPENJAX_TUI_RUNTIME=next`.
- Legacy removal window after switch.

## Validation
- `zsh -lc "cargo build -p tui_next"`
- `zsh -lc "cargo test -p tui_next"`

## Last Updated
2026-03-01

## Document Tree
- `architecture/system-overview.md` (Active)
- `testing/gates.md` (Active)
- `progress/status.md` (Active)
