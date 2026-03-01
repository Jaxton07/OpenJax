# UI/TUI Docs Overview

## Purpose
Provide a single entrypoint for architecture, implementation, testing, and delivery status of the new `ui/tui` runtime.

## Scope
This index covers all documents under `ui/tui/docs` and their ownership status.

## Decisions (Locked)
- `ui/tui` is the legacy/fallback runtime path and remains available during migration.
- `ui/tui_next` is the Codex-core migration target for the next runtime.
- Documentation must follow the fixed section template.
- The default delivery target is Codex-aligned viewport/history architecture.

## Open Questions
- When to physically delete legacy `openjax-tui` and `ui/tui` fallback from workspace.
- When to switch `OPENJAX_TUI_RUNTIME` default from `legacy` to `next`.

## Validation
- Every new/updated doc is linked below with status.
- Milestone and risk sections are updated during every implementation batch.

## Last Updated
2026-03-01

## Milestone Status
- Current milestone: `M2 - Dual-Track Runtime Validation` (Active)

## Top 5 Risks
1. Cursor restore mismatch during history insertion on different terminals.
2. Chinese width wrapping inconsistency between live render and history insert path.
3. Inline viewport height strategy differs from expected Codex behavior on small terminals.
4. Event boundary misclassification for streaming commit points.
5. Insufficient integration tests around resize and overlay interactions.

## Next Actions
1. Complete multi-turn and long-response inline validation for `ui/tui_next`.
2. Implement full scroll key handling (`PageUp/PageDown/Home/End`) and viewport tests.
3. Promote `OPENJAX_TUI_RUNTIME=next` to default only after gating checks pass.

## Runtime Selection
1. Legacy runtime (default): `zsh -lc "cargo run -q -p tui"`
2. Next runtime (dual-track): `zsh -lc "OPENJAX_TUI_RUNTIME=next cargo run -q -p tui"`
3. Direct next crate: `zsh -lc "cargo run -q -p tui_next"`

## Document Tree
- `architecture/system-overview.md` (Active)
- `architecture/runtime-lifecycle.md` (Active)
- `architecture/viewport-history-model.md` (Active)
- `design/render-pipeline.md` (Active)
- `design/input-model.md` (Active)
- `design/style-and-wrap.md` (Active)
- `implementation/module-map.md` (Active)
- `implementation/integration-contracts.md` (Active)
- `implementation/risk-register.md` (Active)
- `progress/changelog.md` (Active)
- `progress/milestones.md` (Active)
- `progress/daily-log.md` (Active)
- `testing/test-strategy.md` (Active)
- `testing/manual-qa-checklist.md` (Active)
- `testing/regression-matrix.md` (Active)
