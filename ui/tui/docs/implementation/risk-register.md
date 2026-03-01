# Risk Register

## Purpose
Track architecture and delivery risks with mitigations.

## Scope
New `ui/tui` runtime delivery.

## Decisions (Locked)
- Track only active engineering risks impacting correctness or schedule.

## Open Questions
- None.

## Validation
- Risks are reviewed and updated per milestone.

## Last Updated
2026-03-01

## Risks
1. Cursor drift after repeated history insertions.
Mitigation: add cursor integrity test and telemetry logs.
2. Wrap inconsistency for CJK lines.
Mitigation: unify wrapping helper and add dedicated regression tests.
3. Duplicate rendering from mixed history/live sources.
Mitigation: enforce exclusive ownership model in app state.
