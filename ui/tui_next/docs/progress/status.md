# Status

## Purpose
Capture current migration status for `tui_next`.

## Scope
Runtime readiness and remaining work items.

## Decisions (Locked)
- Keep `ui/tui` as fallback during dual-track validation.
- Promote `tui_next` to default only after gates pass.

## Open Questions
- Scroll key feature completeness timeline (`PageUp/PageDown/Home/End`).

## Validation
- Current verified:
  - `cargo build -p tui_next`
  - `cargo test -p tui_next`

## Last Updated
2026-03-01

## Snapshot
- Implemented: Codex-core terminal/history/orchestrator migration.
- Implemented: runtime switch `OPENJAX_TUI_RUNTIME=legacy|next`.
- Remaining: final manual stress validation and default switch decision.
