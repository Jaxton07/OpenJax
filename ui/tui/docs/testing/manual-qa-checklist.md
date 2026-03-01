# Manual QA Checklist

## Purpose
Capture manual verification steps before cutover.

## Scope
Interactive runtime behavior in inline and alt-screen modes.

## Decisions (Locked)
- Validate both short and long conversations.
- Validate paste, resize, and scroll keys in each run.

## Open Questions
- Terminal matrix to include (iTerm2, Apple Terminal, tmux).

## Validation
- Checklist completed and linked in milestone exit notes.

## Last Updated
2026-03-01

## Checklist
1. Long session (>200 lines) without overlap or duplicate messages.
2. Paste large text appears immediately.
3. Home/End/PageUp/PageDown behavior stays consistent.
4. CJK text wrapping has no extra spacing drift.
