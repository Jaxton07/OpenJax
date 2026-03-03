# tui

## Purpose
`ui/tui` is the active Codex-core-aligned OpenJax TUI runtime.

## Scope
This crate is now the only maintained Rust UI runtime under `ui/`.

## Decisions (Locked)
- Core terminal stack is based on migrated `terminal/* + insert_history + draw orchestrator`.
- History insertion uses a single queue and a single ANSI insertion path.
- Runtime entry is `cargo run -q -p tui_next`.
- Inline mode keeps shell history visible; viewport anchors to current cursor row and only clamps to bottom when needed.

## Validation
- Build: `zsh -lc "cargo build -p tui_next"`
- Test: `zsh -lc "cargo test -p tui_next"`
- Manual run: `zsh -lc "cargo run -q -p tui_next"`

## Deployment
- Production entrypoint: `tui_next`
- Deployment guide: [../../docs/deployment.md](../../docs/deployment.md)

## Last Updated
2026-03-01

## Architecture Notes
- [Inline Runtime Notes](docs/architecture/inline-runtime-notes.md)
- [System Overview](docs/architecture/system-overview.md)

## File Tree
```text
ui/tui                                   # crate root
├── Cargo.toml                           # crate manifest / deps
├── README.md                            # module overview and run/test guide
├── docs                                 # architecture/progress/test gate docs
│   ├── README.md                        # docs index
│   ├── architecture
│   │   ├── system-overview.md           # runtime boundary and layering
│   │   └── inline-runtime-notes.md      # implementation details, pitfalls, and fixes
│   ├── progress
│   │   └── status.md                    # current migration status snapshot
│   └── testing
│       └── gates.md                     # mandatory quality gates
├── src                                  # runtime implementation
│   ├── app                              # app layer: state transition + render model
│   │   ├── cells.rs                     # history cell builders (user/assistant/tool/system)
│   │   ├── layout_metrics.rs            # visual height calculation and wrapping metrics
│   │   ├── mod.rs                       # App public API and shared helpers
│   │   ├── reducer.rs                   # protocol event -> app state transitions
│   │   ├── render_model.rs              # live/input/footer/approval line assembly
│   │   └── tool_output.rs               # tool output summarization and normalization
│   ├── approval.rs                      # TUI approval handler and pending queue bridge
│   ├── history_cell.rs                  # history cell types and plain constructor
│   ├── input.rs                         # crossterm event -> InputAction mapping
│   ├── insert_history.rs                # ANSI scrollback insertion path
│   ├── lib.rs                           # crate exports and run entry re-export
│   ├── main.rs                          # binary async entry
│   ├── runtime.rs                       # runtime bootstrap and main loop orchestration
│   ├── runtime_loop.rs                  # loop helper functions (drain/render/submit)
│   ├── terminal                         # custom terminal backend and draw pipeline
│   │   ├── core.rs                      # Terminal/Frame lifecycle and viewport/cursor state
│   │   ├── diff.rs                      # buffer diff algorithm to draw commands
│   │   ├── draw.rs                      # ANSI command emission for draw commands
│   │   ├── mod.rs                       # terminal module exports
│   │   ├── style_diff.rs                # shared style modifier diff logic
│   │   └── tests.rs                     # terminal diff regression tests
│   ├── tui.rs                           # high-level TUI composition and draw scheduling
│   ├── wrapping.rs                      # word-wrap helpers for multilingual output
│   └── state                            # persistent app state types
│       ├── app_state.rs                 # AppState + approval/live message structs
│       └── mod.rs                       # state module re-exports
└── tests                                # integration tests (m* quality gates)
    ├── m10_approval_panel_navigation.rs # approval panel navigation behavior
    ├── m1_no_duplicate_history.rs       # no duplicate committed history cells
    ├── m2_cursor_restore_integrity.rs   # history cell identity/cursor-safe assertions
    ├── m3_stream_commit_boundary.rs     # stream-to-commit boundary behavior
    ├── m4_resize_viewport_consistency.rs# viewport/height lower-bound consistency
    ├── m5_chinese_wrap_style_consistency.rs # CJK content style/wrap baseline
    ├── m6_paste_no_typewriter.rs        # paste should append as single action
    ├── m7_startup_banner_once.rs        # startup banner insertion idempotency
    ├── m8_inline_vs_alt_behavior.rs     # inline height env parsing baseline
    └── m9_inline_history_persisted.rs   # final assistant message persistence
```
