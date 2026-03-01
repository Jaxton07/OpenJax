# tui_next

## Purpose
`tui_next` is the Codex-core-aligned next-generation OpenJax TUI runtime used for dual-track migration.

## Scope
This crate contains the next runtime implementation only. The legacy fallback remains in `ui/tui`.

## Decisions (Locked)
- Core terminal stack is based on migrated `custom_terminal + insert_history + draw orchestrator`.
- History insertion uses a single queue and a single ANSI insertion path.
- Runtime selection is controlled by `OPENJAX_TUI_RUNTIME=legacy|next`.

## Open Questions
- When to switch workspace default behavior to `next`.
- Exact removal timeline for legacy runtime.

## Validation
- Build: `zsh -lc "cargo build -p tui_next"`
- Test: `zsh -lc "cargo test -p tui_next"`
- Manual run: `zsh -lc "cargo run -q -p tui_next"`

## Last Updated
2026-03-01
