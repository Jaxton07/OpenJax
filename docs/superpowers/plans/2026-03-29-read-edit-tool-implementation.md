# Read / Edit Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the model-facing `read_file` / `edit_file_range` tools with `Read` / `Edit`, where `Edit` performs single-file unique text replacement instead of line-range editing.

**Architecture:** Keep the existing file-read capability but rename the tool contract exposed to the model to `Read`. Replace the line-number edit path with a new `Edit` handler that takes `file_path`, `old_string`, and `new_string`, normalizes newlines, and succeeds only on a unique match. Align prompt/spec/tool registry/UI display so the entire system exposes one consistent contract.

**Tech Stack:** Rust workspace (`openjax-core`, `ui/tui`), React + TypeScript (`ui/web`), cargo test, pnpm test

---

## File Structure

### Core backend

- Modify: `openjax-core/src/agent/prompt.rs`
  - Replace old tool names and soft guidance with hard `Read` / `Edit` rules.
- Modify: `openjax-core/src/tools/spec.rs`
  - Replace model-facing specs for `read_file` / `edit_file_range` with `Read` / `Edit`.
- Modify: `openjax-core/src/tools/tool_builder.rs`
  - Register new public tool names and remove old default exposure.
- Modify: `openjax-core/src/tools/handlers/mod.rs`
  - Export the new handler modules.
- Create: `openjax-core/src/tools/handlers/edit.rs`
  - Implement unique-match text replacement logic for `Edit`.
- Rename or replace: `openjax-core/src/tools/handlers/read_file.rs`
  - Keep behavior, but clean up public naming so internal module names do not preserve the old external contract.
- Delete: `openjax-core/src/tools/handlers/edit_file_range.rs`
  - Remove the old line-range edit handler after replacement.

### Core tests and docs

- Modify: `openjax-core/src/tests/prompt_and_policy.rs`
  - Assert prompt now mentions `Read` / `Edit` hard rules.
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`
  - Point the suite at the new edit test module.
- Create: `openjax-core/tests/tools_sandbox/m5_edit.rs`
  - Cover `Edit` success and failure semantics.
- Delete: `openjax-core/tests/tools_sandbox/m5_edit_file_range.rs`
  - Remove the line-range test suite.
- Modify: `openjax-core/src/tools/README.md`
- Modify: `openjax-core/src/tools/docs/README.md`
- Modify: `openjax-core/src/tools/docs/tools-list.md`
  - Update user-facing tool documentation to `Read` / `Edit`.

### UI surfaces

- Modify: `ui/tui/src/app/reducer.rs`
  - Ensure TUI displays the backend’s actual `Read` / `Edit` names without old-name assumptions.
- Modify: `ui/web/src/lib/session-events/tools.ts`
  - Keep tool-step titles aligned with backend names and remove any dependency on old names.
- Modify: `ui/web/src/lib/session-events/tools.test.ts`
- Modify: `ui/web/src/components/MessageList.test.tsx`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
- Modify: `ui/web/src/lib/timeline/buildTimeline.test.ts`
  - Replace old tool-name assertions with `Read` / `Edit`.

## Task 1: Lock The Public Contract With Failing Tests

**Files:**
- Modify: `openjax-core/src/tests/prompt_and_policy.rs`
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`
- Create: `openjax-core/tests/tools_sandbox/m5_edit.rs`
- Delete: `openjax-core/tests/tools_sandbox/m5_edit_file_range.rs`
- Modify: `ui/web/src/lib/session-events/tools.test.ts`
- Modify: `ui/web/src/components/MessageList.test.tsx`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
- Modify: `ui/web/src/lib/timeline/buildTimeline.test.ts`

- [ ] **Step 1: Rewrite prompt assertions around `Read` / `Edit`**

Add assertions like:

```rust
assert!(prompt.contains("Modify existing files only after calling `Read`"));
assert!(prompt.contains("Use `Edit` for single-file existing-text edits"));
assert!(!prompt.contains("edit_file_range"));
```

- [ ] **Step 2: Replace the tools sandbox edit suite entry**

Change:

```rust
#[path = "tools_sandbox/m5_edit_file_range.rs"]
mod edit_file_range_m5;
```

to:

```rust
#[path = "tools_sandbox/m5_edit.rs"]
mod edit_m5;
```

- [ ] **Step 3: Write failing `Edit` integration tests**

Cover at minimum:

```rust
#[tokio::test]
async fn edit_replaces_unique_match_successfully() { /* ... */ }

#[tokio::test]
async fn edit_returns_not_found_when_old_string_is_missing() { /* ... */ }

#[tokio::test]
async fn edit_returns_not_unique_for_multiple_matches() { /* ... */ }

#[tokio::test]
async fn edit_normalizes_newlines_before_matching() { /* ... */ }
```

- [ ] **Step 4: Update WebUI tests to assert `Read` / `Edit` titles**

Replace payload/title expectations such as:

```ts
payload: { tool_name: "read_file", target: "README.md" }
```

with:

```ts
payload: { tool_name: "Read", target: "README.md" }
```

and add at least one `Edit` expectation.

- [ ] **Step 5: Run the targeted failing tests**

Run:

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core prompt_and_policy --lib"
zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/tools.test.ts src/components/MessageList.test.tsx src/components/tool-steps/ToolStepCard.test.tsx src/lib/timeline/buildTimeline.test.ts"
```

Expected: failures mentioning missing `Edit` registration/spec behavior and old tool names still being present.

## Task 2: Replace Model-Facing Tool Names In Core

**Files:**
- Modify: `openjax-core/src/agent/prompt.rs`
- Modify: `openjax-core/src/tools/spec.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs`
- Modify: `openjax-core/src/tools/handlers/mod.rs`
- Rename or create: `openjax-core/src/tools/handlers/read.rs`

- [ ] **Step 1: Update prompt rules to a hard `Read` / `Edit` contract**

Target language should look like:

```rust
- Modify existing files only after calling `Read`.
- Use `Edit` for single-file existing-text replacements.
- If `Edit` fails, call `Read` before retrying.
```

- [ ] **Step 2: Replace tool specs**

Define:

```rust
pub fn create_read_spec() -> ToolSpec { /* name: "Read" */ }
pub fn create_edit_spec() -> ToolSpec { /* name: "Edit" */ }
```

and remove default model-facing `edit_file_range`.

- [ ] **Step 3: Register the new public names**

Update `build_tool_registry_with_config` so the registry exposes:

```rust
builder.register_handler("Read", Arc::new(ReadHandler));
builder.register_handler("Edit", Arc::new(EditHandler));
```

Keep internal naming consistent with the new contract instead of leaving old public aliases in place.

- [ ] **Step 4: Update registry/spec unit tests**

Adjust assertions from:

```rust
assert!(names.contains(&"read_file".to_string()));
```

to:

```rust
assert!(names.contains(&"Read".to_string()));
assert!(names.contains(&"Edit".to_string()));
```

- [ ] **Step 5: Run focused core tests**

Run:

```bash
zsh -lc "cargo test -p openjax-core prompt_and_policy --lib"
zsh -lc "cargo test -p openjax-core tool_builder --lib"
```

Expected: PASS.

## Task 3: Implement The New `Edit` Semantics

**Files:**
- Create: `openjax-core/src/tools/handlers/edit.rs`
- Delete: `openjax-core/src/tools/handlers/edit_file_range.rs`
- Modify: `openjax-core/src/tools/handlers/mod.rs`
- Modify: `openjax-core/tests/tools_sandbox/m5_edit.rs`

- [ ] **Step 1: Implement argument parsing for `Edit`**

Use a struct like:

```rust
#[derive(Deserialize)]
struct EditArgs {
    file_path: String,
    old_string: String,
    new_string: String,
}
```

Reject empty `file_path` or empty `old_string` as `invalid_args`.

- [ ] **Step 2: Implement newline-normalized unique matching**

Implementation shape:

```rust
let original = tokio::fs::read_to_string(&path).await?;
let normalized_file = normalize_newlines(&original);
let normalized_old = normalize_newlines(&args.old_string);
let matches = find_all_ranges(&normalized_file, &normalized_old);
```

Rules:
- `0` matches -> `not_found`
- `>1` matches -> `not_unique`
- `1` match -> replace that single range with `new_string`

- [ ] **Step 3: Preserve original newline style on write**

If the source file uses `\r\n`, convert the final output back to `\r\n` before writing so newline normalization only affects matching, not file style.

- [ ] **Step 4: Return compact success/failure text**

Success shape:

```text
The file <path> has been updated successfully.
```

Failure shape examples:

```text
Edit failed [not_found]: old_string was not found in <path>. Call Read before retrying.
Edit failed [not_unique]: old_string matched multiple locations in <path>. Call Read and provide a more specific old_string.
```

- [ ] **Step 5: Run the edit integration suite**

Run:

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
```

Expected: PASS.

## Task 4: Align TUI, WebUI, And Tool Docs

**Files:**
- Modify: `ui/tui/src/app/reducer.rs`
- Modify: `ui/web/src/lib/session-events/tools.ts`
- Modify: `ui/web/src/lib/session-events/tools.test.ts`
- Modify: `ui/web/src/components/MessageList.test.tsx`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
- Modify: `ui/web/src/lib/timeline/buildTimeline.test.ts`
- Modify: `openjax-core/src/tools/README.md`
- Modify: `openjax-core/src/tools/docs/README.md`
- Modify: `openjax-core/src/tools/docs/tools-list.md`

- [ ] **Step 1: Remove old-name display assumptions from TUI**

Verify `display_name` fallback and history cell text continue to work when backend now emits `Read` / `Edit`.

- [ ] **Step 2: Make Web tool-step titles reflect the real backend names**

Ensure `createStepFromEvent` continues to use:

```ts
payload.display_name ?? String(payload.tool_name ?? "tool")
```

and update tests so the UI contract no longer depends on `read_file`.

- [ ] **Step 3: Rewrite tool docs to the new contract**

Update examples from:

```bash
tool:read_file file_path=src/lib.rs
tool:edit_file_range file_path=src/lib.rs start_line=10 end_line=12 new_text='...'
```

to:

```bash
tool:Read file_path=src/lib.rs
tool:Edit file_path=src/lib.rs old_string='old' new_string='new'
```

- [ ] **Step 4: Run UI and doc-adjacent tests**

Run:

```bash
zsh -lc "cargo test -p tui_next"
zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/tools.test.ts src/components/MessageList.test.tsx src/components/tool-steps/ToolStepCard.test.tsx src/lib/timeline/buildTimeline.test.ts"
```

Expected: PASS.

## Task 5: Full Verification And Cleanup

**Files:**
- Modify: any files touched above

- [ ] **Step 1: Search for stale public references**

Run:

```bash
zsh -lc "rg -n \"read_file|edit_file_range\" openjax-core ui/tui ui/web"
```

Expected: only intentional historical/spec/archive references remain.

- [ ] **Step 2: Run final focused verification**

Run:

```bash
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --lib"
zsh -lc "cargo test -p tui_next"
zsh -lc "cd ui/web && pnpm test"
```

Expected: PASS.

- [ ] **Step 3: Commit in small logical slices**

Recommended commit sequence:

```bash
git add openjax-core/src/tests/prompt_and_policy.rs openjax-core/tests/tools_sandbox_suite.rs openjax-core/tests/tools_sandbox/m5_edit.rs
git commit -m "test(core): 定义 Read 与 Edit 工具契约"

git add openjax-core/src/agent/prompt.rs openjax-core/src/tools/spec.rs openjax-core/src/tools/tool_builder.rs openjax-core/src/tools/handlers
git commit -m "feat(core): 重构 Read 与 Edit 工具接口"

git add ui/tui/src/app/reducer.rs ui/web/src/lib/session-events/tools.ts ui/web/src/lib/session-events/tools.test.ts ui/web/src/components/MessageList.test.tsx ui/web/src/components/tool-steps/ToolStepCard.test.tsx ui/web/src/lib/timeline/buildTimeline.test.ts openjax-core/src/tools/README.md openjax-core/src/tools/docs/README.md openjax-core/src/tools/docs/tools-list.md
git commit -m "refactor(ui,docs): 对齐 Read 与 Edit 展示和文档"
```
