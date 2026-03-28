# Native Tool Calling Remaining Phases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 收口 Native Tool Calling Phase 6 文档/清理项，并保留 Phase 4-5 的执行记录。

**Architecture:** 以已落地的 Phase 3 native loop + Phase 4 工具补齐 + Phase 5 shell 输出分离为基线，继续保持 `planner_tool_action.rs` 独立，完成文档和收尾验证对齐。

**Tech Stack:** Rust 2024, Cargo, tokio, serde/serde_json, openjax-protocol events, openjax-core tools/sandbox/planner modules

**Spec:** `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md`

---

## Execution Status (2026-03-28)

- 基线提交：`4fb54fa2`（Phase 4-5 代码与回归收口）
- 当前阶段：Task 8（Phase 6 docs and cleanup alignment）
- 计划定位：Task 1-7 保留为历史执行记录；后续以 Task 8/9 的收尾验证为准

---

## File Change Map

### Phase 4

- Modify: `openjax-core/src/tools/handlers/mod.rs`
- Modify: `openjax-core/src/tools/spec.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs`
- Modify: `openjax-core/src/tools/README.md` if the tool list needs updating in the same batch
- Create: `openjax-core/src/tools/handlers/write_file.rs`
- Create: `openjax-core/src/tools/handlers/glob_files.rs`
- Create: `openjax-core/tests/tools_sandbox/m10_write_file.rs`
- Create: `openjax-core/tests/tools_sandbox/m11_glob_files.rs`
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`
- Modify: `openjax-core/Cargo.toml` if `glob` crate support is needed

### Phase 5

- Modify: `openjax-protocol/src/lib.rs`
- Modify: `openjax-core/src/tools/router_impl.rs`
- Modify: `openjax-core/src/sandbox/mod.rs`
- Modify: `openjax-core/src/agent/planner.rs`
- Modify: `openjax-core/src/agent/planner_tool_action.rs`
- Modify: `openjax-core/src/tests/streaming.rs`
- Modify: `openjax-core/src/tests/support.rs`
- Modify: `openjax-core/src/tests/tool_batch_approval.rs`
- Modify: `openjax-core/tests/streaming_suite.rs` and/or underlying `openjax-core/tests/streaming/*` cases if event assertions move there
- Modify: `openjax-core/tests/tools_sandbox/*` if shell behavior assertions belong in integration coverage

### Phase 6

- Modify: `openjax-core/README.md`
- Modify: `openjax-core/src/agent/README.md`
- Modify: `openjax-core/src/tools/README.md` if Phase 4/5 behavior changed the documented contract
- Modify: `docs/plan/refactor/tools/native-tool-calling-plan.md`
- Modify: `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md`
- Modify: `docs/superpowers/plans/2026-03-28-native-tool-calling-remaining-phases.md`
- Modify: any remaining `openjax-core` code comments or module docs that still describe the old JSON planner path as current

## Phase Boundaries

- Phase 4 is tool-surface work only. Do not change planner loop semantics in this phase.
- Phase 5 is output-contract work. Do not add unrelated tools or planner refactors here.
- Phase 6 is cleanup and verification only. Do not reopen architecture decisions that the spec already fixed.

---

### Task 1: Phase 4 test scaffolding for `write_file` and `glob_files`

**Files:**
- Create: `openjax-core/tests/tools_sandbox/m10_write_file.rs`
- Create: `openjax-core/tests/tools_sandbox/m11_glob_files.rs`
- Modify: `openjax-core/tests/tools_sandbox_suite.rs`
- Reference: `openjax-core/tests/tools_sandbox/m4_apply_patch.rs`
- Reference: `openjax-core/tests/tools_sandbox/m5_edit_file_range.rs`
- Reference: `openjax-core/tests/tools_sandbox/m9_system_tools.rs`

- [ ] **Step 1: Add the new suite entries**

Update `openjax-core/tests/tools_sandbox_suite.rs` to include the new files:

```rust
#[path = "tools_sandbox/m10_write_file.rs"]
mod write_file_m10;
#[path = "tools_sandbox/m11_glob_files.rs"]
mod glob_files_m11;
```

- [ ] **Step 2: Write failing `write_file` integration coverage**

Create `openjax-core/tests/tools_sandbox/m10_write_file.rs` with cases for:

```rust
#[tokio::test]
async fn write_file_creates_new_file_inside_workspace() { /* ... */ }

#[tokio::test]
async fn write_file_overwrites_existing_file() { /* ... */ }

#[tokio::test]
async fn write_file_rejects_workspace_escape() { /* ... */ }

#[tokio::test]
async fn write_file_creates_missing_parent_directories() { /* ... */ }
```

- [ ] **Step 3: Write failing `glob_files` integration coverage**

Create `openjax-core/tests/tools_sandbox/m11_glob_files.rs` with cases for:

```rust
#[tokio::test]
async fn glob_files_returns_matches_sorted_newest_first() { /* ... */ }

#[tokio::test]
async fn glob_files_rejects_workspace_escape() { /* ... */ }

#[tokio::test]
async fn glob_files_respects_limit() { /* ... */ }

#[tokio::test]
async fn glob_files_returns_empty_when_nothing_matches() { /* ... */ }
```

- [ ] **Step 4: Run the new suite cases to confirm they fail**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite write_file -- --nocapture"`
Expected: FAIL because `write_file` is not registered yet

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite glob_files -- --nocapture"`
Expected: FAIL because `glob_files` is not registered yet

- [ ] **Step 5: Commit the failing-test scaffold**

```bash
git add openjax-core/tests/tools_sandbox_suite.rs openjax-core/tests/tools_sandbox/m10_write_file.rs openjax-core/tests/tools_sandbox/m11_glob_files.rs
git commit -m "test(core): 补充 native tool calling phase4 工具测试骨架"
```

---

### Task 2: Implement `write_file` and register it cleanly

**Files:**
- Create: `openjax-core/src/tools/handlers/write_file.rs`
- Modify: `openjax-core/src/tools/handlers/mod.rs`
- Modify: `openjax-core/src/tools/spec.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs`
- Test: `openjax-core/tests/tools_sandbox/m10_write_file.rs`

- [ ] **Step 1: Define the failing handler contract**

Model the handler after the existing file-mutation tools and keep the args shape explicit:

```rust
#[derive(Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

pub struct WriteFileHandler;
```

- [ ] **Step 2: Run the targeted test again before implementation**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite write_file_creates_new_file_inside_workspace -- --nocapture"`
Expected: FAIL with unknown tool or missing handler/spec behavior

- [ ] **Step 3: Implement the minimal `write_file` handler**

Implement `openjax-core/src/tools/handlers/write_file.rs` with:

```rust
// Parse JSON args -> validate workspace-relative path -> create parent dirs -> overwrite file
// Return a short success string such as:
format!("written {} ({} bytes)", file_path.display(), content.len())
```

Requirements:

- Reject workspace escape
- Create parent directories automatically
- Overwrite existing content directly
- Avoid embedding planner-specific logic in the handler

- [ ] **Step 4: Register the handler and tool spec**

Add to `openjax-core/src/tools/handlers/mod.rs`:

```rust
pub mod write_file;
pub use write_file::WriteFileHandler;
```

Add `create_write_file_spec()` to `openjax-core/src/tools/spec.rs` with required fields:

```rust
{
  "file_path": { "type": "string" },
  "content": { "type": "string" }
}
```

Register it in `openjax-core/src/tools/tool_builder.rs` in both the spec build path and handler registration path.

- [ ] **Step 5: Run the focused tests until they pass**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite write_file -- --nocapture"`
Expected: PASS for all `write_file` cases

- [ ] **Step 6: Commit `write_file`**

```bash
git add openjax-core/src/tools/handlers/write_file.rs openjax-core/src/tools/handlers/mod.rs openjax-core/src/tools/spec.rs openjax-core/src/tools/tool_builder.rs openjax-core/tests/tools_sandbox/m10_write_file.rs openjax-core/tests/tools_sandbox_suite.rs
git commit -m "feat(core): 新增 write_file 工具"
```

---

### Task 3: Implement `glob_files` and move `apply_patch` format detail into the tool spec

**Files:**
- Create: `openjax-core/src/tools/handlers/glob_files.rs`
- Modify: `openjax-core/src/tools/handlers/mod.rs`
- Modify: `openjax-core/src/tools/spec.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs`
- Modify: `openjax-core/Cargo.toml`
- Test: `openjax-core/tests/tools_sandbox/m11_glob_files.rs`
- Reference: `openjax-core/src/agent/prompt.rs`

- [ ] **Step 1: Add the failing `glob_files` contract**

Use an explicit args struct:

```rust
#[derive(Deserialize)]
struct GlobFilesArgs {
    pattern: String,
    base_path: Option<String>,
    limit: usize,
}
```

- [ ] **Step 2: Run the targeted `glob_files` test before implementation**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite glob_files -- --nocapture"`
Expected: FAIL with unknown tool or missing behavior

- [ ] **Step 3: Add any required crate dependency**

If `glob` is not already available, add it to `openjax-core/Cargo.toml`:

```toml
glob = "0.3"
```

- [ ] **Step 4: Implement `glob_files` minimally**

Create `openjax-core/src/tools/handlers/glob_files.rs` to:

- resolve `base_path` against the workspace root
- reject path escape
- evaluate the glob pattern
- sort matches by modification time descending
- enforce `limit`
- return one path per line

- [ ] **Step 5: Register the new tool and update specs**

Add the handler export in `openjax-core/src/tools/handlers/mod.rs`, then add `create_glob_files_spec()` in `openjax-core/src/tools/spec.rs`, and register it in `openjax-core/src/tools/tool_builder.rs`.

At the same time, move the detailed `apply_patch` format contract out of planner prompt instructions and into the `apply_patch` tool description in `openjax-core/src/tools/spec.rs`.

- [ ] **Step 6: Keep prompt-side guidance short**

Adjust `openjax-core/src/agent/prompt.rs` only if needed so it keeps tool-selection policy, not duplicated patch grammar prose.

- [ ] **Step 7: Run focused tool coverage**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite glob_files -- --nocapture"`
Expected: PASS for all `glob_files` cases

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite system_tools_are_registered_in_specs -- --nocapture"`
Expected: PASS, confirming the broader registry/spec surface still behaves

- [ ] **Step 8: Commit `glob_files` and spec cleanup**

```bash
git add openjax-core/Cargo.toml openjax-core/src/tools/handlers/glob_files.rs openjax-core/src/tools/handlers/mod.rs openjax-core/src/tools/spec.rs openjax-core/src/tools/tool_builder.rs openjax-core/src/agent/prompt.rs openjax-core/tests/tools_sandbox/m11_glob_files.rs openjax-core/tests/tools_sandbox_suite.rs
git commit -m "feat(core): 补齐 glob_files 与 apply_patch 描述归位"
```

---

### Task 4: Add Phase 5 failing tests for shell output separation

**Files:**
- Modify: `openjax-core/src/tests/streaming.rs`
- Modify: `openjax-core/src/tests/tool_batch_approval.rs`
- Modify: `openjax-core/src/tests/support.rs`
- Modify: `openjax-core/tests/streaming_suite.rs` and/or `openjax-core/tests/streaming/*` only if integration coverage is a better fit
- Reference: `openjax-protocol/src/lib.rs`
- Reference: `openjax-core/src/tools/router_impl.rs`
- Reference: `openjax-core/src/sandbox/mod.rs`

- [ ] **Step 1: Add crate-internal tests for output contract separation**

Add assertions that the shell execution contract distinguishes model content from event/display output:

```rust
#[test]
fn tool_exec_outcome_keeps_model_content_separate_from_display_output() { /* ... */ }

#[tokio::test]
async fn native_tool_result_uses_model_content_not_display_output() { /* ... */ }
```

- [ ] **Step 2: Add event-shape assertions**

Add or update tests to assert that `ToolCallCompleted` carries structured shell metadata instead of requiring display-output parsing:

```rust
assert!(matches!(event, Event::ToolCallCompleted { shell_metadata: Some(_), .. }));
```

- [ ] **Step 3: Run the focused tests to confirm they fail**

Run: `zsh -lc "cargo test -p openjax-core --lib streaming -- --nocapture"`
Expected: FAIL because the old contract still mixes display and model content

Run: `zsh -lc "cargo test -p openjax-core --lib tool_batch_approval -- --nocapture"`
Expected: FAIL on missing metadata or stale event shape

- [ ] **Step 4: Commit the failing-test batch**

```bash
git add openjax-core/src/tests/streaming.rs openjax-core/src/tests/tool_batch_approval.rs openjax-core/src/tests/support.rs
git commit -m "test(core): 补充 shell 输出分离回归测试"
```

---

### Task 5: Extend protocol and router outcome types for shell metadata

**Files:**
- Modify: `openjax-protocol/src/lib.rs`
- Modify: `openjax-core/src/tools/router_impl.rs`
- Modify: `openjax-core/src/tools/tool_builder.rs` only if helper constructors need shape updates
- Test: `openjax-core/src/tests/streaming.rs`

- [ ] **Step 1: Add the protocol type first**

Extend `openjax-protocol/src/lib.rs` with a structured shell metadata payload:

```rust
pub struct ShellExecutionMetadata {
    pub result_class: String,
    pub backend: String,
    pub exit_code: i32,
    pub policy_decision: String,
    pub runtime_allowed: bool,
    pub degrade_reason: Option<String>,
    pub runtime_deny_reason: Option<String>,
}
```

Then update `Event::ToolCallCompleted` to carry:

```rust
shell_metadata: Option<ShellExecutionMetadata>
```

- [ ] **Step 2: Run compile to catch protocol fan-out early**

Run: `zsh -lc "cargo build -p openjax-protocol && cargo build -p openjax-core"`
Expected: FAIL in `openjax-core` until router/sandbox/planner call sites are updated

- [ ] **Step 3: Update the core execution outcome contract**

Modify `openjax-core/src/tools/router_impl.rs` so the shell/tool execution return type distinguishes:

```rust
pub struct ToolExecOutcome {
    pub model_content: String,
    pub display_output: String,
    pub shell_metadata: Option<ShellExecutionMetadata>,
    pub success: bool,
}
```

Keep non-shell tools simple: `model_content == display_output`, `shell_metadata == None`.

- [ ] **Step 4: Update or add unit tests for the new outcome shape**

Run: `zsh -lc "cargo test -p openjax-core --lib tool_exec_outcome_keeps_model_content_separate_from_display_output -- --nocapture"`
Expected: PASS

- [ ] **Step 5: Commit protocol and router contract changes**

```bash
git add openjax-protocol/src/lib.rs openjax-core/src/tools/router_impl.rs openjax-core/src/tests/streaming.rs
git commit -m "refactor(core): 引入 shell 结构化输出元数据"
```

---

### Task 6: Update sandbox and native planner execution to consume the split contract

**Files:**
- Modify: `openjax-core/src/sandbox/mod.rs`
- Modify: `openjax-core/src/agent/planner.rs`
- Modify: `openjax-core/src/agent/planner_tool_action.rs`
- Modify: `openjax-core/src/tests/support.rs`
- Modify: `openjax-core/src/tests/streaming.rs`
- Modify: `openjax-core/src/tests/tool_batch_approval.rs`

- [ ] **Step 1: Refactor shell execution to build both content channels**

In `openjax-core/src/sandbox/mod.rs`, produce:

```rust
let model_content = format!("exit_code={}\nstdout:\n{}\nstderr:\n{}", ...);
let display_output = format!("result_class={}\ncommand={}\n...", ...);
let shell_metadata = ShellExecutionMetadata { ... };
```

Only the structured `model_content` should be fed back into model-facing tool results.

- [ ] **Step 2: Run the targeted crate tests before planner wiring**

Run: `zsh -lc "cargo test -p openjax-core --lib native_tool_result_uses_model_content_not_display_output -- --nocapture"`
Expected: FAIL until planner/planner_tool_action consume `model_content`

- [ ] **Step 3: Update native tool execution in `planner_tool_action.rs`**

When `execute_native_tool_call` receives a tool outcome, make sure it:

- appends `model_content` to `UserContentBlock::ToolResult`
- emits `ToolCallCompleted.output` using `display_output`
- includes `shell_metadata` in the completed event
- preserves existing guard, approval, duplicate-detection, and loop-abort behavior

- [ ] **Step 4: Update any planner-side direct tool result usage**

In `openjax-core/src/agent/planner.rs`, ensure any tool-result construction or trace recording uses the correct channel:

- model loop uses `model_content`
- event/display reporting uses `display_output`
- `tool_traces` remain human-meaningful summaries, not raw metadata dumps

- [ ] **Step 5: Run focused crate tests until they pass**

Run: `zsh -lc "cargo test -p openjax-core --lib streaming -- --nocapture"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --lib tool_batch_approval -- --nocapture"`
Expected: PASS

- [ ] **Step 6: Run focused integration coverage**

Run: `zsh -lc "cargo test -p openjax-core --test streaming_suite -- --nocapture"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite -- --nocapture"`
Expected: PASS

- [ ] **Step 7: Commit Phase 5 execution wiring**

```bash
git add openjax-core/src/sandbox/mod.rs openjax-core/src/agent/planner.rs openjax-core/src/agent/planner_tool_action.rs openjax-core/src/tests/support.rs openjax-core/src/tests/streaming.rs openjax-core/src/tests/tool_batch_approval.rs openjax-core/tests/streaming_suite.rs openjax-core/tests/approval_events_suite.rs
git commit -m "refactor(core): 分离 shell 模型输出与展示输出"
```

---

### Task 7: Run Phase 4-5 regression commands and fix fallout before docs cleanup

**Files:**
- No new files expected
- Modify only the files exposed by verification failures

- [ ] **Step 1: Run build and no-run checks**

Run: `zsh -lc "cargo build -p openjax-core"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --no-run"`
Expected: PASS

- [ ] **Step 2: Run focused suites in the order most likely to catch Phase 4-5 regressions**

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test streaming_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test approval_events_suite"`
Expected: PASS

Run: `zsh -lc "cargo test -p openjax-core --test core_history_suite"`
Expected: PASS

- [ ] **Step 3: Fix only verification-exposed issues**

If failures appear, patch only the files directly implicated by test output. Do not opportunistically refactor unrelated modules here.

- [ ] **Step 4: Commit the regression fixes**

```bash
git add <exact-files-from-failures>
git commit -m "fix(core): 收口 native tool calling 剩余阶段回归问题"
```

---

### Task 8: Phase 6 docs and cleanup alignment

**Files:**
- Modify: `openjax-core/README.md`
- Modify: `openjax-core/src/agent/README.md`
- Modify: `openjax-core/src/tools/README.md`
- Modify: `docs/plan/refactor/tools/native-tool-calling-plan.md`
- Modify: `docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md`
- Modify: `docs/superpowers/plans/2026-03-28-native-tool-calling-remaining-phases.md`
- Modify: any remaining `openjax-core` docs/comments that still describe the JSON planner path as current

- [ ] **Step 1: Audit docs for outdated planner language**

Search for stale references to the JSON planner as the current path:

Run: `zsh -lc "rg -n \"DecisionJsonStreamParser|parse_model_decision|planner prompt|JSON planner|fallback-to-complete\" openjax-core docs/plan/refactor/tools docs/superpowers"`
Expected: a small, reviewable set of remaining references

- [ ] **Step 2: Update README and module docs to match the actual architecture**

Make sure the docs say:

- Phase 3 native loop is already complete
- `planner_tool_action.rs` remains intentionally independent
- `write_file` and `glob_files` are part of the supported tool surface
- shell result semantics are split between model content and display metadata

- [ ] **Step 3: Mark the old total migration plan as historical context**

Update `docs/plan/refactor/tools/native-tool-calling-plan.md` so it clearly points future work to the remaining-phases spec/plan instead of reading like the active implementation baseline.

- [ ] **Step 4: Keep cleanup bounded**

Delete only code or docs that are clearly no longer part of the active path. If a leftover helper still has tests, references, or explanatory value, mark it as non-primary rather than deleting it in this pass.

- [ ] **Step 5: Run doc-adjacent validation**

Run: `zsh -lc "cargo test -p openjax-core --lib prompt_and_policy -- --nocapture"`
Expected: PASS if prompt-side guidance changed

Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`
Expected: PASS if tool docs/spec text moved

- [ ] **Step 6: Commit docs and cleanup alignment**

```bash
git add openjax-core/README.md openjax-core/src/agent/README.md openjax-core/src/tools/README.md docs/plan/refactor/tools/native-tool-calling-plan.md docs/superpowers/specs/2026-03-28-native-tool-calling-remaining-phases-design.md docs/superpowers/plans/2026-03-28-native-tool-calling-remaining-phases.md
git commit -m "📝 docs(core): 对齐 native tool calling 剩余阶段收口文档"
```

---

### Task 9: Final closure verification for the remaining phases

**Files:**
- No code changes expected unless verification exposes a real issue

- [ ] **Step 1: Run the final `openjax-core` integration set**

Run: `zsh -lc "cargo test -p openjax-core --tests"`
Expected: PASS

- [ ] **Step 2: Run crate-internal tests if any Phase 4-6 unit modules changed**

Run: `zsh -lc "cargo test -p openjax-core --lib"`
Expected: PASS

- [ ] **Step 3: Record exact verification evidence**

Capture:

- command
- PASS/FAIL
- any skipped command with a concrete reason

Do not claim the remaining phases are fully complete without real command output.

- [ ] **Step 4: Commit only if verification required last-mile fixes**

```bash
git add <exact-files-if-needed>
git commit -m "fix(core): 收口 native tool calling 最终验证问题"
```

---

## Completion Checklist

- [ ] `write_file` is implemented, registered, and covered by integration tests
- [ ] `glob_files` is implemented, registered, and covered by integration tests
- [ ] `apply_patch` format detail lives in the tool spec instead of being duplicated in planner prompt prose
- [ ] shell execution returns separate `model_content` and `display_output`
- [ ] `ToolCallCompleted` carries structured shell metadata
- [ ] native planner tool results use model-facing content only
- [ ] regression suites for tools, streaming, approval, and history pass
- [ ] README/spec/plan/doc language matches the current native tool calling architecture
- [ ] final `openjax-core` test evidence is recorded before calling the work complete
