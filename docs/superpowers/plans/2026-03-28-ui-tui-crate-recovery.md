# UI TUI Crate Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore `ui/tui` as a normal Rust crate so `cargo build -p tui_next` and `cargo test -p tui_next --test ...` can resolve `tui_next::...` before any `shell_metadata` UI work.

**Architecture:** Reintroduce the deleted crate entry layer and the minimal internal support modules that the current `ui/tui/src/*` files still import. Do not change WebUI, gateway, or core behavior; first recover compile/test wiring, then use existing TUI tests to separate crate-base failures from real business failures.

**Tech Stack:** Rust 2024, Cargo workspace, `ratatui`, `crossterm`, `tokio`

---

### Task 1: Reproduce and confirm the crate-base root cause

**Files:**
- Modify: `docs/superpowers/plans/2026-03-28-ui-tui-crate-recovery.md`
- Check: `ui/tui/Cargo.toml`
- Check: `ui/tui/tests/m12_tool_partial_status.rs`
- Check: `ui/tui/tests/m17_degraded_mutating_warning.rs`

- [ ] **Step 1: Use the existing failing tests as the RED state**

Run: `zsh -lc "cargo test -p tui_next --test m12_tool_partial_status -- --nocapture"`
Expected: fail before assertions because `tui_next` cannot be resolved.

Run: `zsh -lc "cargo test -p tui_next --test m17_degraded_mutating_warning -- --nocapture"`
Expected: fail before assertions because `tui_next` cannot be resolved.

- [ ] **Step 2: Confirm Cargo target shape is broken**

Run: `zsh -lc "cargo metadata --no-deps --format-version 1"`
Expected: `tui_next` package exists but has only `tests/*` targets and no `lib` or `bin`.

- [ ] **Step 3: Confirm which current source files still depend on deleted modules**

Check imports from:
- `ui/tui/src/app/mod.rs`
- `ui/tui/src/runtime.rs`
- `ui/tui/src/tui.rs`

Expected: current source still imports `approval`, `history_cell`, `input`, `insert_history`, `terminal`, and `wrapping`.

### Task 2: Restore the minimum crate entry and support modules

**Files:**
- Create: `ui/tui/src/lib.rs`
- Create: `ui/tui/src/main.rs`
- Create: `ui/tui/src/approval.rs`
- Create: `ui/tui/src/history_cell.rs`
- Create: `ui/tui/src/input.rs`
- Create: `ui/tui/src/insert_history.rs`
- Create: `ui/tui/src/wrapping.rs`
- Create: `ui/tui/src/terminal/mod.rs`
- Create: `ui/tui/src/terminal/core.rs`
- Create: `ui/tui/src/terminal/diff.rs`
- Create: `ui/tui/src/terminal/draw.rs`
- Create: `ui/tui/src/terminal/style_diff.rs`
- Create: `ui/tui/src/app/cells.rs`

- [ ] **Step 1: Recreate the crate entry layer**

Add `ui/tui/src/lib.rs` to expose the current TUI modules and provide `pub async fn run() -> anyhow::Result<()>`.

Add `ui/tui/src/main.rs` to call `tui_next::run()`.

- [ ] **Step 2: Recreate the missing internal modules that current code imports**

Restore the deleted support files listed above from the last known pre-deletion state, then keep them aligned with the current module graph instead of doing unrelated refactors.

- [ ] **Step 3: Keep the change minimal**

Do not modify protocol/core/gateway/WebUI files.
Do not change TUI behavior beyond what is required to restore compilation and existing test reachability.

### Task 3: Verify crate recovery and identify remaining real failures

**Files:**
- Check: `ui/tui/tests/*.rs`

- [ ] **Step 1: Verify the crate target exists**

Run: `zsh -lc "cargo metadata --no-deps --format-version 1"`
Expected: `tui_next` now includes at least a `lib` target, and likely a `bin` target.

- [ ] **Step 2: Run the narrow failing tests again**

Run: `zsh -lc "cargo test -p tui_next --test m12_tool_partial_status -- --nocapture"`

Run: `zsh -lc "cargo test -p tui_next --test m17_degraded_mutating_warning -- --nocapture"`

Expected: failures, if any, should now be business/API mismatches rather than unresolved crate errors.

- [ ] **Step 3: Run broader TUI verification**

Run: `zsh -lc "cargo build -p tui_next"`

Run: `zsh -lc "cargo test -p tui_next"`

Expected: either green, or a reduced failure set that can be reported as the remaining true TUI issues.
