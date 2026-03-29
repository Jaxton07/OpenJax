# Temporarily Hide `apply_patch` from Model Exposure

## Background

`apply_patch` currently supports Add, Update, Delete, Move, and Rename file operations, with a multi-level fuzzy matching algorithm and a ~9-module internal implementation under `openjax-core/src/tools/apply_patch/`. While capable, it is perceived as over-engineered for daily use. We want to trial a simpler workflow where the model relies on `Edit` (single-file text replacement) and `shell` (for delete/move/rename) instead.

## Goal

- Remove `apply_patch` from the model-visible tool surface.
- Ensure policy, approval, and sandbox layers treat `apply_patch` as non-existent.
- Keep the source code and original tests in the repository for easy rollback.
- Do not touch `Edit`, `Read`, `shell`, or other tools.

## Non-Goal

- Delete `apply_patch` source code or its unit/integration tests.
- Introduce a new replacement tool.
- Modify `apply_patch`'s internal behavior.

## Selected Approach: `ApplyPatchToolType::Disabled`

We follow the same pattern already used for `ShellToolType::Disabled`:

1. Add a `Disabled` variant to `ApplyPatchToolType`.
2. Set `ToolsConfig::default().apply_patch_tool_type` to `Some(Disabled)`.
3. In `build_all_specs`, skip emitting the `apply_patch` spec when disabled.
4. In `build_tool_registry_with_config`, skip registering `ApplyPatchHandler` when disabled.
5. Scrub all prompt, planner, and tool-guard references so the model is never told to use `apply_patch`.
6. Temporarily disable `apply_patch` integration tests from the suite runner.
7. Update documentation listings to reflect the current supported tool surface.

## Detailed Changes

### Configuration & Registration

Files:
- `openjax-core/src/tools/spec.rs`
  - Add `Disabled` to `ApplyPatchToolType`.
  - Change `ToolsConfig::default()` to `Some(ApplyPatchToolType::Disabled)`.
  - In `build_all_specs`, guard the `apply_patch` spec behind `!= Some(Disabled)`.
- `openjax-core/src/tools/tool_builder.rs`
  - In `build_tool_registry_with_config`, guard `ApplyPatchHandler` registration behind the same condition.
  - Update existing unit tests to assert `apply_patch` is absent by default.

### Prompt & Decision Layer

Files:
- `openjax-core/src/agent/prompt.rs`
  - Remove `apply_patch` from system prompt and planner prompt tool-selection rules.
  - Replace guidance with: multi-file edits or file operations (add/delete/move/rename) should use `shell` or `Edit`.
- `openjax-core/src/agent/planner_utils.rs`
  - Remove `"apply_patch"` from `extract_tool_target_hint` match arms and `is_mutating_tool`.
- `openjax-core/src/agent/decision.rs`
  - Remove `"apply_patch"` from `canonical_tool_name` mapping so JSON planner mode also rejects it.
- `openjax-core/src/agent/tool_guard.rs`
  - Remove `ApplyPatchReadGuard` and its enum entirely.
- `openjax-core/src/agent/planner.rs`
  - Remove `apply_patch_read_guard` from `ToolActionContext` and all usage sites.

### Interceptor (preserved)

- `openjax-core/src/tools/apply_patch_interceptor.rs`
  - Leave code intact. Because `apply_patch` is no longer a registered tool, the interceptor will only fire if a user manually types `apply_patch` into `shell`, which is acceptable for the trial period.

### Tests

- `openjax-core/tests/tools_sandbox_suite.rs`
  - Comment out `mod apply_patch_m4;`.
- `openjax-core/tests/tools_sandbox/m4_apply_patch.rs`
  - Prefix the entire file with `#[ignore = "apply_patch temporarily hidden from model"]` or comment out module contents for easy restoration.
- `openjax-core/tests/policy_center_suite.rs` and related suites
  - Update any assertions that assume `apply_patch` is present in the default tool set.

### Documentation

- `openjax-core/src/tools/README.md`
  - Remove `apply_patch` from the "当前受支持工具面" bullet list.
- `openjax-core/src/tools/docs/tools-list.md`
  - Remove the `apply_patch` tool listing from the supported tools table, but keep the detailed `apply_patch/` architecture section further down the doc (as historical/implementation reference).

## Error Handling / Rollback

If the model somehow still emits `apply_patch` (e.g. cached prompt or old context), it will fail because:
1. `canonical_tool_name` no longer maps it.
2. `ToolRegistry` has no handler for it.
The failure message will be a standard unsupported-tool error.

## Testing Plan

- `cargo test -p openjax-core --test tools_sandbox_suite` passes without `apply_patch` tests.
- `cargo test -p openjax-core --test skills_suite` passes.
- `cargo test -p openjax-core --test approval_suite` passes.
- Scoped grep in `openjax-core/src/agent/` and `openjax-core/src/tools/` shows no registered `apply_patch` spec/handler by default.

## Acceptance Criteria

- `build_default_tool_registry()` does not include an `apply_patch` spec.
- `build_default_tool_registry()` does not register an `ApplyPatchHandler`.
- Prompts no longer mention `apply_patch` as a usable tool.
- `ApplyPatchReadGuard` is removed.
- `apply_patch` integration tests are skipped/not compiled into the active suite.
- All remaining core tests pass.
