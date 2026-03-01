# Status

## Purpose
Capture current migration status for `tui_next`.

## Scope
Runtime readiness and remaining work items.

## Decisions (Locked)
- `tui_next` is the only maintained runtime in `ui/`.
- Promote quality gates before additional feature customization.

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
- Implemented: tool output compact summary UI and approval panel options.
- Remaining: final manual stress validation and package naming cleanup decision.
