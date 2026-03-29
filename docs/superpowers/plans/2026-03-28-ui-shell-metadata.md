# UI Shell Metadata Adaptation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `ui/tui` 与 `ui/web` 都正确消费 `ToolCallCompleted.shell_metadata`，完成 shell 结构化结果的 UI 适配。

**Architecture:** 分两阶段执行。Phase 1 先修 TUI 的协议跟进与结构化消费，保持字符串解析为 fallback；Phase 2 再给 WebUI 补类型、reducer 和卡片展示，复用 gateway 已透传的 payload。

**Tech Stack:** Rust 2024, openjax-protocol events, ratatui TUI, React + TypeScript + Vitest

**Spec:** `docs/superpowers/specs/2026-03-28-ui-shell-metadata-design.md`

---

## File Change Map

### Phase 1: TUI

- Modify: `ui/tui/src/app/reducer.rs`
- Modify: `ui/tui/src/app/tool_output.rs`
- Modify: `ui/tui/tests/m12_tool_partial_status.rs`
- Modify: `ui/tui/tests/m17_degraded_mutating_warning.rs`
- Modify: other `ui/tui/tests/*` files that still construct old `ToolCallCompleted`

### Phase 2: WebUI

- Modify: `ui/web/src/types/gateway.ts`
- Modify: `ui/web/src/types/chat.ts`
- Modify: `ui/web/src/lib/session-events/tools.ts`
- Modify: `ui/web/src/lib/session-events/tools.test.ts`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.tsx`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
- Modify: related `MessageList` or timeline tests only if rendering assertions need adjustment

---

### Task 1: Lock failing TUI protocol-shape tests

**Files:**
- Modify: `ui/tui/tests/m12_tool_partial_status.rs`
- Modify: `ui/tui/tests/m17_degraded_mutating_warning.rs`
- Search: `ui/tui/tests/*`

- [ ] **Step 1: Update failing test fixtures to include `shell_metadata`**

For each `Event::ToolCallCompleted` test fixture, add:

```rust
shell_metadata: Some(ShellExecutionMetadata {
    result_class: "...".to_string(),
    backend: "...".to_string(),
    exit_code: 0,
    policy_decision: "...".to_string(),
    runtime_allowed: true,
    degrade_reason: None,
    runtime_deny_reason: None,
}),
```

- [ ] **Step 2: Run focused TUI tests to verify RED or compile-fix boundary**

Run:

- `zsh -lc "cargo test -p tui_next --test m12_tool_partial_status"`
- `zsh -lc "cargo test -p tui_next --test m17_degraded_mutating_warning"`

Expected:

- compile succeeds on the new event shape
- assertions still fail or remain incomplete until structured consumption is implemented

---

### Task 2: Implement TUI structured shell metadata consumption

**Files:**
- Modify: `ui/tui/src/app/reducer.rs`
- Modify: `ui/tui/src/app/tool_output.rs`

- [ ] **Step 1: Write the failing TUI behavior tests**

Add or update tests so they assert behavior from `shell_metadata` rather than relying only on text parsing:

- partial success text derives from `result_class`
- sandbox backend text derives from `backend`
- degraded warning derives from `backend + policy_decision + degrade_reason`
- skill-trigger hint derives from `runtime_deny_reason`

- [ ] **Step 2: Run targeted TUI tests and verify RED**

Run the narrowest relevant `cargo test -p tui_next --test ...` commands.

Expected: FAIL because current reducer/tool_output still rely on `output` text only.

- [ ] **Step 3: Thread `display_name` and `shell_metadata` through completed event rendering**

Update `ui/tui/src/app/reducer.rs` so `ToolCallCompleted` passes structured fields into the rendering path.

- [ ] **Step 4: Refactor `tool_output.rs` to prefer metadata and fallback to text parsing**

Implement helpers that:

- accept `shell_metadata`
- derive backend / degraded / deny hint / partial summary from metadata first
- fallback to parsing `output` only when metadata is absent

- [ ] **Step 5: Run focused TUI tests to verify GREEN**

Run:

- `zsh -lc "cargo test -p tui_next --test m12_tool_partial_status"`
- `zsh -lc "cargo test -p tui_next --test m17_degraded_mutating_warning"`

Expected: PASS

---

### Task 3: Run full TUI regression

**Files:**
- Test-only validation

- [ ] **Step 1: Run crate-level TUI regression**

Run:

- `zsh -lc "cargo test -p tui_next"`

Expected: PASS

- [ ] **Step 2: Record any remaining old-shape fixture files**

If additional TUI tests fail due to missing `shell_metadata`, update them in the same batch.

---

### Task 4: Lock failing WebUI reducer tests

**Files:**
- Modify: `ui/web/src/lib/session-events/tools.test.ts`
- Modify: `ui/web/src/types/gateway.ts`

- [ ] **Step 1: Add reducer tests for structured shell metadata**

Add cases for:

```ts
payload: {
  tool_call_id: "call_1",
  tool_name: "shell",
  ok: true,
  output: "ok",
  shell_metadata: {
    result_class: "partial_success",
    backend: "none_escalated",
    exit_code: 0,
    policy_decision: "AskApproval",
    runtime_allowed: true,
    degrade_reason: "macos denied",
    runtime_deny_reason: null
  }
}
```

Assert the reducer produces a `ToolStep` that retains enough structured info for rendering.

- [ ] **Step 2: Run focused WebUI tests to verify RED**

Run:

- `zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/tools.test.ts"`

Expected: FAIL because reducer currently ignores `shell_metadata`.

---

### Task 5: Implement WebUI types and reducer mapping

**Files:**
- Modify: `ui/web/src/types/gateway.ts`
- Modify: `ui/web/src/types/chat.ts`
- Modify: `ui/web/src/lib/session-events/tools.ts`

- [ ] **Step 1: Add explicit shell metadata types**

Define `ShellExecutionMetadata` in `ui/web/src/types/gateway.ts`.

- [ ] **Step 2: Add the minimum local ToolStep fields needed for rendering**

If needed, extend `ToolStep` with stable fields for:

- backend summary
- degraded summary
- deny hint
- partial status description

Prefer keeping raw payload in `meta` and only add dedicated fields that materially simplify rendering.

- [ ] **Step 3: Update reducer to derive display semantics from metadata**

For `tool_call_completed`:

- do not blindly hardcode `status: "success"`
- derive descriptive text from `shell_metadata`
- keep `output` for raw detail view

- [ ] **Step 4: Run focused reducer tests to verify GREEN**

Run:

- `zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/tools.test.ts"`

Expected: PASS

---

### Task 6: Implement WebUI card rendering

**Files:**
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.tsx`
- Modify: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`

- [ ] **Step 1: Add failing component tests**

Cover:

- backend summary visible
- degraded/risk warning visible
- runtime deny hint visible
- partial success message visible

- [ ] **Step 2: Run focused component tests to verify RED**

Run:

- `zsh -lc "cd ui/web && pnpm test -- src/components/tool-steps/ToolStepCard.test.tsx"`

Expected: FAIL because card currently only renders `description/code/output`.

- [ ] **Step 3: Implement the minimum rendering changes**

Update `ToolStepCard` to render the metadata-derived summaries without redesigning the card layout.

- [ ] **Step 4: Re-run focused component tests to verify GREEN**

Run:

- `zsh -lc "cd ui/web && pnpm test -- src/components/tool-steps/ToolStepCard.test.tsx"`

Expected: PASS

---

### Task 7: Run full WebUI regression

**Files:**
- Test-only validation

- [ ] **Step 1: Run WebUI test suite**

Run:

- `zsh -lc "cd ui/web && pnpm test"`

Expected: PASS

- [ ] **Step 2: Run WebUI build**

Run:

- `zsh -lc "cd ui/web && pnpm build"`

Expected: PASS

---

### Task 8: Final cross-check and docs

**Files:**
- Modify: `ui/web/README.md` if shell metadata UI behavior needs documentation
- Modify: `docs/superpowers/specs/2026-03-28-ui-shell-metadata-design.md` if implementation changed details
- Modify: `docs/superpowers/plans/2026-03-28-ui-shell-metadata.md` with final verification notes if needed

- [ ] **Step 1: Update any consumer docs that still imply output-string parsing is the only UI source**

- [ ] **Step 2: Record final verification commands and results**

- [ ] **Step 3: Commit in two phases if practical**

Recommended commit split:

1. `feat(tui): 优先消费 shell_metadata`
2. `feat(web): 消费 tool_call_completed shell_metadata`
