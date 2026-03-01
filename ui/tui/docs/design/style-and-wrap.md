# Style and Wrap

## Purpose
Define style reset and wrapping strategy for consistent output.

## Scope
`insert_history.rs` and live rendering text shaping.

## Decisions (Locked)
- Wrap is width-aware with unicode width handling.
- Style is reset at line end to avoid leakage.
- History insertion preserves line and span style intent.

## Open Questions
- Future markdown styling parity with Codex renderer.

## Validation
- Chinese and mixed-width test cases must be included in regression.

## Last Updated
2026-03-01
