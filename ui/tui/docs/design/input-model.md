# Input Model

## Purpose
Define keyboard/paste behavior and side effects.

## Scope
`ui/tui/src/input.rs` and `App` input handlers.

## Decisions (Locked)
- Paste event is appended as a whole string.
- No character-by-character typing animation.
- Enter submits the full input buffer.

## Open Questions
- Multi-line input mode support timing.

## Validation
- Tests must assert paste action is emitted as a single append.

## Last Updated
2026-03-01
