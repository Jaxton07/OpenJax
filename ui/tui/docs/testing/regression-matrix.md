# Regression Matrix

## Purpose
List scenarios that must stay green after each change.

## Scope
History insertion, viewport updates, style consistency, and input handling.

## Decisions (Locked)
- Every critical bug class has a matching regression case.

## Open Questions
- None.

## Validation
- Matrix maps to automated tests and manual checklist steps.

## Last Updated
2026-03-01

## Cases
1. Duplicate render prevention.
2. Cursor restore after repeated insertions.
3. Streaming commit boundary correctness.
4. Resize stability.
5. CJK wrap + style persistence.
6. Paste without typewriter effect.
