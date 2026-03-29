# OpenJax Core Src Tests Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `openjax-core/src/tests.rs` into focused internal test modules without changing the existing `openjax-core/tests/` integration suite structure or behavior.

**Architecture:** Convert the current single-file `#[cfg(test)] mod tests;` implementation from a file module into a directory module at `openjax-core/src/tests/mod.rs`. Move shared test doubles into a dedicated `support.rs`, keep topic-specific assertions in narrow submodules, and preserve all tests as crate-internal unit tests so private-item access remains unchanged.

**Tech Stack:** Rust 2024, Cargo test, crate-internal unit test modules, existing `openjax-core` test doubles and async `tokio::test` coverage.

---

## Scope And Constraints

- Keep `openjax-core/src/lib.rs` using `#[cfg(test)] mod tests;` with no public API changes.
- Do not move these tests into `openjax-core/tests/`; those are integration suites with different visibility and compile boundaries.
- Do not change the semantics of existing tests unless a helper is provably dead and can be safely removed.
- Prefer the smallest clean split that leaves each file focused and comfortably readable.
- If a helper is used by only one submodule after the split, keep it local instead of promoting it into `support.rs`.

## Target File Structure

### Existing

- Modify: `openjax-core/src/tests.rs`

### New

- Create: `openjax-core/src/tests/mod.rs`
- Create: `openjax-core/src/tests/support.rs`
- Create: `openjax-core/src/tests/prompt_and_policy.rs`
- Create: `openjax-core/src/tests/duplicate_guard.rs`
- Create: `openjax-core/src/tests/streaming.rs`
- Create: `openjax-core/src/tests/tool_batch_approval.rs`

## File Responsibilities

### `openjax-core/src/tests/mod.rs`

- Declares the internal test submodules.
- Re-exports nothing publicly.
- Keeps only the minimum shared imports or module wiring required by the submodules.

### `openjax-core/src/tests/support.rs`

- Holds shared test fixtures and helpers used by multiple internal test files.
- Expected contents:
  - `text_response`
  - `ScriptedStreamingModel`
  - `ScriptedToolBatchModel`
  - `ScriptedToolBatchDependencyModel`
  - `NativeStreamingFinalModel`
  - `NativeStreamingToolUseModel`
  - `DuplicateToolLoopModel`
  - `ApprovalBlockedBatchModel`
  - `ApprovalCancellationBatchModel`
  - `SlowProbeTool`
  - `RejectApprovalHandler`
- Review `PlannerFallbackModel`; remove it if it is no longer referenced after the split.

### `openjax-core/src/tests/prompt_and_policy.rs`

- Keeps pure unit tests around prompt construction and runtime policy resolution.
- Expected tests:
  - `parse_runtime_policies`
  - `resolves_turn_limits_from_config_and_env_with_precedence`
  - `build_system_prompt_contains_verification_rule`
  - `build_system_prompt_contains_skills_section`
  - `build_turn_messages_includes_prior_conversation_summary`
  - `refresh_loop_recovery_only_updates_last_user_text`
  - `summarize_user_input_escapes_control_newlines`
  - `summarize_user_input_adds_ellipsis_when_truncated`

### `openjax-core/src/tests/duplicate_guard.rs`

- Keeps duplicate tool detection and duplicate-loop abort behavior together.
- Expected tests:
  - `duplicate_detection_is_turn_local_when_cleared`
  - `duplicate_detection_resets_after_mutation_epoch_change`
  - `aborts_after_consecutive_duplicate_skips`
  - `duplicate_tool_skip_and_abort_emit_response_error_events`

### `openjax-core/src/tests/streaming.rs`

- Keeps response streaming and native-streaming event ordering tests together.
- Expected tests:
  - `final_action_emits_response_text_delta_before_completion`
  - `planner_only_mode_skips_final_writer_and_keeps_response_delta_events`
  - `planner_only_mode_with_stream_engine_v2_still_skips_final_writer`
  - `native_streaming_final_response_does_not_fallback_to_complete`
  - `planner_stream_tool_events_preserve_tool_name_across_args_delta_and_ready`

### `openjax-core/src/tests/tool_batch_approval.rs`

- Keeps tool batch execution, approval interruption, and cancellation behavior together.
- Expected tests:
  - `tool_batch_emits_proposal_and_batch_completed_events`
  - `tool_batch_dependency_unmet_still_emits_started_before_completed`
  - `tool_batch_approval_blocked_stops_followup_scheduling_and_rounds`
  - `tool_batch_approval_blocked_cancels_pending_parallel_tool`

## Migration Strategy

### Task 1: Introduce the directory test module shell

**Files:**
- Delete: `openjax-core/src/tests.rs`
- Create: `openjax-core/src/tests/mod.rs`

- [ ] **Step 1: Create the new directory module entrypoint**

Add `mod.rs` with:

```rust
mod duplicate_guard;
mod prompt_and_policy;
mod streaming;
mod support;
mod tool_batch_approval;
```

- [ ] **Step 2: Remove the old single-file module**

Delete `openjax-core/src/tests.rs` only after its contents have been redistributed.

- [ ] **Step 3: Verify `lib.rs` needs no semantic change**

Keep:

```rust
#[cfg(test)]
mod tests;
```

Expected: Rust resolves `tests` to the new directory module automatically.

### Task 2: Extract shared fixtures into `support.rs`

**Files:**
- Create: `openjax-core/src/tests/support.rs`

- [ ] **Step 1: Move only truly shared imports and helpers**

Bring over the imports needed by the moved test doubles and helper functions, including async traits, event/model types, tool traits, filesystem helpers, and policy helpers.

- [ ] **Step 2: Move multi-file fixtures into `support.rs`**

Copy the current shared structs and `impl`s, preserving names first to minimize churn.

- [ ] **Step 3: Make fixture visibility crate-local to sibling modules**

Use `pub(super)` where sibling test modules need access:

```rust
pub(super) struct ScriptedStreamingModel { ... }
pub(super) fn text_response(...) -> ModelResponse { ... }
```

- [ ] **Step 4: Prune dead fixtures**

Search for references after the move. If `PlannerFallbackModel` has no test references, remove it instead of carrying it forward.

### Task 3: Split pure prompt and policy tests

**Files:**
- Create: `openjax-core/src/tests/prompt_and_policy.rs`

- [ ] **Step 1: Move pure synchronous tests first**

Move the prompt and runtime policy tests that do not depend on async fixtures or tool execution.

- [ ] **Step 2: Keep imports local and narrow**

Import only the prompt builders, runtime policy helpers, `Agent`, `Config`, `SandboxMode`, and conversation/history types needed by these tests.

- [ ] **Step 3: Preserve assertion wording**

Do not rewrite assertions unless needed for imports or module paths.

### Task 4: Split duplicate guard behavior tests

**Files:**
- Create: `openjax-core/src/tests/duplicate_guard.rs`

- [ ] **Step 1: Move duplicate bookkeeping tests**

Move the two `Agent::record_tool_call` tests and the duplicate skip threshold test.

- [ ] **Step 2: Move duplicate loop async coverage**

Move `duplicate_tool_skip_and_abort_emit_response_error_events` and import `DuplicateToolLoopModel` from `support`.

- [ ] **Step 3: Keep duplicate-specific helpers out of unrelated modules**

If any helper ends up unique to this file, keep it here rather than in `support.rs`.

### Task 5: Split streaming behavior tests

**Files:**
- Create: `openjax-core/src/tests/streaming.rs`

- [ ] **Step 1: Move response delta ordering tests**

Move both tests that assert `ResponseTextDelta` comes before `ResponseCompleted`.

- [ ] **Step 2: Move native streaming no-fallback coverage**

Move the tests using `NativeStreamingFinalModel` and `NativeStreamingToolUseModel`.

- [ ] **Step 3: Keep shared model fixtures in `support.rs`**

Import the scripted/native streaming fixtures from `support` rather than duplicating them.

### Task 6: Split tool batch and approval behavior tests

**Files:**
- Create: `openjax-core/src/tests/tool_batch_approval.rs`

- [ ] **Step 1: Move tool batch proposal/dependency tests**

Move the two tests around tool batch proposal, started/completed ordering, and dependency-unmet behavior.

- [ ] **Step 2: Move approval-blocked tests**

Move the two approval-blocked tests and import `ApprovalBlockedBatchModel`, `ApprovalCancellationBatchModel`, `RejectApprovalHandler`, and `SlowProbeTool` from `support`.

- [ ] **Step 3: Preserve filesystem cleanup behavior**

Keep the temporary workspace setup and cleanup logic unchanged unless a safer local helper extraction clearly improves readability.

### Task 7: Compile-fix module visibility and imports

**Files:**
- Modify: `openjax-core/src/tests/mod.rs`
- Modify: `openjax-core/src/tests/support.rs`
- Modify: each new submodule file as needed

- [ ] **Step 1: Resolve sibling-module visibility issues**

Use `pub(super)` for support items used across sibling test modules.

- [ ] **Step 2: Resolve `super::` path changes carefully**

Because these files now live under `crate::tests::<module>`, update paths so they still refer to the crate root or sibling support module correctly.

- [ ] **Step 3: Keep imports explicit**

Prefer explicit `use crate::...` or `use super::support::...` imports instead of broad globs.

### Task 8: Verification

**Files:**
- No code changes expected unless verification exposes an issue

- [ ] **Step 1: Run focused crate tests**

Run:

```bash
zsh -lc "cargo test -p openjax-core --lib tests::prompt_and_policy"
zsh -lc "cargo test -p openjax-core --lib tests::duplicate_guard"
zsh -lc "cargo test -p openjax-core --lib tests::streaming"
zsh -lc "cargo test -p openjax-core --lib tests::tool_batch_approval"
```

Expected: all moved internal test modules compile and pass.

- [ ] **Step 2: Run full library tests for openjax-core**

Run:

```bash
zsh -lc "cargo test -p openjax-core --lib"
```

Expected: crate-internal tests pass with no regressions from module reorganization.

- [ ] **Step 3: Run at least one existing integration suite as a boundary check**

Run:

```bash
zsh -lc "cargo test -p openjax-core --test streaming_suite"
```

Expected: confirms the `openjax-core/tests/` integration suite remains unaffected by the internal test split.

## Risks To Watch

- Sibling test modules cannot see each other’s private items; missing `pub(super)` on shared fixtures will cause compile failures.
- Path updates from the old monolithic `tests.rs` can accidentally refer to the wrong `super`; verify every `super::` import after the split.
- Moving tests out of one file can expose dead fixtures that were previously tolerated; remove them deliberately rather than masking unused-code warnings.
- If a new test file starts growing beyond a few hundred lines after the split, it should be re-sliced again rather than rebuilding the monolith in directory form.

## Definition Of Done

- `openjax-core/src/tests.rs` no longer exists.
- Internal tests live under `openjax-core/src/tests/` with clear responsibility boundaries.
- Existing `openjax-core/tests/` integration suites remain unchanged.
- `cargo test -p openjax-core --lib` passes.
- `cargo test -p openjax-core --test streaming_suite` passes as a boundary check.
