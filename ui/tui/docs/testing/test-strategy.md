# Test Strategy

## Purpose
Define automated test coverage for runtime correctness.

## Scope
Unit tests and integration tests under `ui/tui/tests`.

## Decisions (Locked)
- Prioritize invariants: no duplicates, cursor integrity, wrap consistency.
- Include explicit scenarios for CJK width and resize behavior.

## Open Questions
- Snapshot framework choice for layout assertions.

## Validation
- `cargo test -p tui` passes in CI and locally.

## Last Updated
2026-03-01
