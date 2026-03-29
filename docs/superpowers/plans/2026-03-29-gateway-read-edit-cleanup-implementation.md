# Gateway Read / Edit Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the Read / Edit migration by removing remaining old public tool-name references from `openjax-gateway` and deleting the unused legacy `openjax-core/src/tools/read_file.rs`.

**Architecture:** Keep the change surface narrow. Update gateway activity/tests to use `Read` / `Edit`, remove the dead legacy core helper file, then verify with scoped grep plus targeted Rust tests that no active code or tests still expose `read_file` / `edit_file_range`.

**Tech Stack:** Rust workspace, Axum gateway tests, `cargo test`, `rg`

---

## File Map

- Modify: `openjax-gateway/src/stdio/dispatch.rs`
- Modify: `openjax-gateway/tests/policy_api/m5_policy_effect.rs`
- Modify: `openjax-core/src/tools/mod.rs` only if export cleanup is required after file removal
- Delete: `openjax-core/src/tools/read_file.rs`
- Verify: `openjax-gateway`, `openjax-core/src/tools`, `openjax-core/tests`

### Task 1: Update Gateway Old Tool-Name References

**Files:**
- Modify: `openjax-gateway/src/stdio/dispatch.rs`
- Modify: `openjax-gateway/tests/policy_api/m5_policy_effect.rs`

- [ ] **Step 1: Read the exact target sections before editing**

Run:

```bash
zsh -lc "sed -n '1180,1245p' openjax-gateway/src/stdio/dispatch.rs"
zsh -lc "sed -n '1,180p' openjax-gateway/tests/policy_api/m5_policy_effect.rs"
```

Expected: See remaining `read_file` literals in gateway test/sample paths.

- [ ] **Step 2: Update gateway literals to `Read` / `Edit`**

Make these minimal edits:

- In `openjax-gateway/src/stdio/dispatch.rs`, change the test event sample tool name from `read_file` to `Read`
- In `openjax-gateway/tests/policy_api/m5_policy_effect.rs`, change:
  - rule id `read_file_gate` to `read_gate`
  - route path `/api/v1/policy/rules/read_file_gate` to `/api/v1/policy/rules/read_gate`
  - `tool_name` values from `read_file` to `Read`
  - turn input from `tool:read_file path=Cargo.toml` to `tool:Read file_path=Cargo.toml`
  - expected matched rule id from `read_file_gate` to `read_gate`

- [ ] **Step 3: Run focused gateway tests**

Run:

```bash
zsh -lc "cargo test -p openjax-gateway --lib"
zsh -lc "cargo test -p openjax-gateway --test policy_api_suite policy_rule_create_update_publish_affects_submit_turn"
```

Expected: PASS.

### Task 2: Remove Dead Legacy Core Read File

**Files:**
- Modify: `openjax-core/src/tools/mod.rs` only if needed
- Delete: `openjax-core/src/tools/read_file.rs`

- [ ] **Step 1: Confirm the file has no remaining callers**

Run:

```bash
zsh -lc "rg -n 'mod read_file|read_file::|crate::tools::read_file|tools/read_file.rs|read_file\\(' openjax-core/src openjax-core/tests -g '!**/docs/**'"
```

Expected: Only hits from `openjax-core/src/tools/read_file.rs` itself.

- [ ] **Step 2: Delete the unused legacy file and any stale module/export references**

Make these minimal edits:

- Delete `openjax-core/src/tools/read_file.rs`
- If compilation shows stale references, remove them from `openjax-core/src/tools/mod.rs` or related module wiring

- [ ] **Step 3: Run focused core tests**

Run:

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --lib"
```

Expected: PASS.

### Task 3: Scoped Cleanup Verification

**Files:**
- Verify only

- [ ] **Step 1: Run scoped grep for old public tool names**

Run:

```bash
zsh -lc "rg -n 'read_file|edit_file_range' openjax-core openjax-gateway -g '!**/docs/archive/**' -g '!**/docs/superpowers/**'"
```

Expected: No hits in active code/tests. If hits remain, they must be either newly discovered active leftovers to fix now or clearly out-of-scope historical docs to exclude more narrowly.

- [ ] **Step 2: Run gateway and core verification together**

Run:

```bash
zsh -lc "cargo test -p openjax-gateway --test policy_api_suite"
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
```

Expected: PASS.

- [ ] **Step 3: Review diff before handoff**

Run:

```bash
zsh -lc "git diff -- openjax-gateway openjax-core/src/tools docs/superpowers/specs/2026-03-29-gateway-read-edit-cleanup-design.md docs/superpowers/plans/2026-03-29-gateway-read-edit-cleanup-implementation.md"
```

Expected: Only the intended gateway cleanup, legacy file deletion, and design/plan docs appear.
