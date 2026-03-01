# tui_next

## Purpose
`tui_next` is the active Codex-core-aligned OpenJax TUI runtime.

## Scope
This crate is now the only maintained Rust UI runtime under `ui/`.

## Decisions (Locked)
- Core terminal stack is based on migrated `custom_terminal + insert_history + draw orchestrator`.
- History insertion uses a single queue and a single ANSI insertion path.
- Runtime entry is `cargo run -q -p tui_next`.

## Open Questions
- Whether to rename package `tui_next` to `tui` in a follow-up cleanup.

## Validation
- Build: `zsh -lc "cargo build -p tui_next"`
- Test: `zsh -lc "cargo test -p tui_next"`
- Manual run: `zsh -lc "cargo run -q -p tui_next"`

## Last Updated
2026-03-01

## File Tree
```text
ui/tui_next
├── Cargo.toml
├── README.md
├── docs
│   ├── README.md
│   ├── architecture
│   │   └── system-overview.md
│   ├── progress
│   │   └── status.md
│   └── testing
│       └── gates.md
├── src
│   ├── app.rs
│   ├── approval.rs
│   ├── custom_terminal.rs
│   ├── history_cell.rs
│   ├── input.rs
│   ├── insert_history.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── runtime.rs
│   ├── tui.rs
│   ├── wrapping.rs
│   └── state
│       ├── app_state.rs
│       └── mod.rs
└── tests
    ├── m10_approval_panel_navigation.rs
    ├── m1_no_duplicate_history.rs
    ├── m2_cursor_restore_integrity.rs
    ├── m3_stream_commit_boundary.rs
    ├── m4_resize_viewport_consistency.rs
    ├── m5_chinese_wrap_style_consistency.rs
    ├── m6_paste_no_typewriter.rs
    ├── m7_startup_banner_once.rs
    ├── m8_inline_vs_alt_behavior.rs
    └── m9_inline_history_persisted.rs
```
