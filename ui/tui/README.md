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
│   │   ├── slash_palette.rs             # slash palette state transitions and accept logic
│   │   └── tool_output.rs               # tool output summarization and normalization
│   ├── approval.rs                      # TUI approval handler and pending queue bridge
│   ├── history_cell.rs                  # history cell types and plain constructor
│   ├── input.rs                         # crossterm event -> InputAction mapping
│   ├── insert_history.rs                # ANSI scrollback insertion path
│   ├── lib.rs                           # crate exports and run entry re-export
│   ├── main.rs                          # binary async entry
│   ├── runtime.rs                       # runtime bootstrap: init agent, app, tui and drive main loop
│   ├── runtime_loop.rs                  # loop helper functions (drain/render/submit)
│   ├── slash_commands.rs                # slash command registry and match/sort logic
│   ├── status                           # status bar rendering and animation
│   │   ├── indicator.rs                 # status line assembly with elapsed timer
│   │   ├── mod.rs                       # status module exports
│   │   └── shimmer.rs                   # shimmer/wave animation for status label
│   ├── terminal                         # custom terminal backend and draw pipeline
│   │   ├── core.rs                      # Terminal/Frame lifecycle and viewport/cursor state
│   │   ├── diff.rs                      # buffer diff algorithm to draw commands
│   │   ├── draw.rs                      # ANSI command emission for draw commands
│   │   ├── mod.rs                       # terminal module exports
│   │   ├── style_diff.rs                # shared style modifier diff logic
│   │   └── tests.rs                     # terminal diff regression tests
│   ├── tui.rs                           # high-level TUI composition and draw scheduling
│   ├── viewport.rs                      # viewport plan, layout constraints and transient clipping
│   ├── wrapping.rs                      # word-wrap helpers for multilingual output
│   └── state                            # persistent app state types
│       ├── app_state.rs                 # AppState + approval/live message structs
│       └── mod.rs                       # state module re-exports
└── tests                                # integration tests (m* quality gates)
    ├── m10_approval_panel_navigation.rs # approval panel navigation behavior
    ├── m11_shell_target_visibility.rs   # shell tool target visibility
    ├── m12_tool_partial_status.rs       # partial tool status rendering
    ├── m13_input_navigation.rs           # input cursor/history navigation
    ├── m14_shell_approval_panel_copy.rs # approval panel copy contract
    ├── m15_approval_dedup.rs             # approval request dedup and resolve behavior
    ├── m16_shell_multiline_target.rs     # multiline shell target sanitization
    ├── m17_degraded_mutating_warning.rs  # degraded mutating risk warning
    ├── m18_status_bar_replaces_live_status.rs # status bar replaces transient live status
    ├── m19_status_bar_clears_on_turn_complete.rs # status bar clears on turn completion
    ├── m1_no_duplicate_history.rs       # no duplicate committed history cells
    ├── m20_submit_sets_status_running.rs # submit starts running status
    ├── m21_slash_palette_behavior.rs    # slash palette interactions and key mappings
    ├── m22_bottom_layout_stability.rs   # bottom layout remains stable with transient UI
    ├── m2_cursor_restore_integrity.rs   # history cell identity/cursor-safe assertions
    ├── m3_stream_commit_boundary.rs     # stream-to-commit boundary behavior
    ├── m4_resize_viewport_consistency.rs# viewport/height lower-bound consistency
    ├── m5_chinese_wrap_style_consistency.rs # CJK content style/wrap baseline
    ├── m6_paste_no_typewriter.rs        # paste should append as single action
    ├── m7_startup_banner_once.rs        # startup banner insertion idempotency
    ├── m8_inline_vs_alt_behavior.rs     # inline height env parsing baseline
    └── m9_inline_history_persisted.rs   # final assistant message persistence
```
