# Integration Contracts

## Purpose
Define boundaries between TUI and core/protocol systems.

## Scope
Public methods and event contracts needed for runtime integration.

## Decisions (Locked)
- No public API changes to `openjax-core` or `openjax-protocol`.
- New TUI consumes existing protocol events through adapter layer.

## Open Questions
- Exact event mapping table for streaming tokens in first integration pass.

## Validation
- Build and test in workspace without protocol/core modifications.

## Last Updated
2026-03-01
