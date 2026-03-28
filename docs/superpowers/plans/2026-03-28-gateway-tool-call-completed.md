# Gateway ToolCallCompleted Shell Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `openjax-gateway` 的 `tool_call_completed` 在 SSE / timeline / stdio 三条出口上完整透传 `shell_metadata`，并统一 payload 口径。

**Architecture:** 保持 gateway 现有分层不变，直接修正 `event_mapper` 与 stdio 独立映射代码；timeline 继续依赖 `state/events.rs` 的通用 payload 落盘链路，不新增特殊分支。

**Tech Stack:** Rust 2024, tokio, serde_json, openjax-protocol events, axum test harness

**Spec:** `docs/superpowers/specs/2026-03-28-gateway-tool-call-completed-design.md`

---

## File Change Map

- Modify: `openjax-gateway/src/event_mapper/tool.rs`
- Modify: `openjax-gateway/src/event_mapper/mod.rs` tests
- Modify: `openjax-gateway/src/stdio/dispatch.rs`
- Modify: `openjax-gateway/src/state/events.rs` tests only if timeline verification belongs there
- Modify: `openjax-gateway/tests/gateway_api.rs` only if there is a cheap integration path to assert timeline payload
- Modify: `openjax-gateway/README.md`

## Task 1: Lock the failing gateway mapper tests

**Files:**
- Modify: `openjax-gateway/src/event_mapper/mod.rs`
- Reference: `openjax-gateway/src/event_mapper/tool.rs`

- [ ] **Step 1: Add a `ToolCallCompleted` mapper test with structured shell metadata**

Add a focused test that constructs:

```rust
Event::ToolCallCompleted {
    turn_id: 1,
    tool_call_id: "call_1".to_string(),
    tool_name: "shell".to_string(),
    ok: true,
    output: "done".to_string(),
    shell_metadata: Some(ShellExecutionMetadata { /* ... */ }),
    display_name: Some("Run Shell".to_string()),
}
```

Assert the mapped payload contains:

```rust
payload["tool_call_id"] == "call_1"
payload["display_name"] == "Run Shell"
payload["shell_metadata"]["backend"] == "sandbox"
```

- [ ] **Step 2: Run the targeted mapper test and verify RED**

Run: `zsh -lc "cargo test -p openjax-gateway event_mapper -- --nocapture"`

Expected: FAIL because `tool_call_completed` payload currently omits `shell_metadata`.

## Task 2: Lock the failing stdio mapping tests

**Files:**
- Modify: `openjax-gateway/src/stdio/dispatch.rs`

- [ ] **Step 1: Add a focused stdio `map_event` test for `ToolCallCompleted`**

Construct the same protocol event and assert the stdio envelope payload contains:

```rust
payload["tool_call_id"] == "call_1"
payload["display_name"] == "Run Shell"
payload["shell_metadata"]["backend"] == "sandbox"
```

- [ ] **Step 2: Run the targeted stdio test and verify RED**

Run: `zsh -lc "cargo test -p openjax-gateway stdio::dispatch -- --nocapture"`

Expected: FAIL because stdio currently omits `tool_call_id`, `display_name`, and `shell_metadata`.

## Task 3: Implement the minimal mapper fixes

**Files:**
- Modify: `openjax-gateway/src/event_mapper/tool.rs`
- Modify: `openjax-gateway/src/stdio/dispatch.rs`

- [ ] **Step 1: Extend the SSE/timeline mapper**

Update `Event::ToolCallCompleted` mapping in `openjax-gateway/src/event_mapper/tool.rs` to emit:

```rust
json!({
    "tool_call_id": tool_call_id,
    "tool_name": tool_name,
    "ok": ok,
    "output": output,
    "shell_metadata": shell_metadata,
    "display_name": display_name,
})
```

- [ ] **Step 2: Extend the stdio mapper to the same payload contract**

Update `map_event` in `openjax-gateway/src/stdio/dispatch.rs` so `tool_call_completed` emits the same fields, and `tool_call_started` also carries `tool_call_id` / `display_name` to reduce divergence on the tool lifecycle contract.

- [ ] **Step 3: Re-run focused tests and verify GREEN**

Run:

- `zsh -lc "cargo test -p openjax-gateway event_mapper -- --nocapture"`
- `zsh -lc "cargo test -p openjax-gateway stdio::dispatch -- --nocapture"`

Expected: PASS

## Task 4: Prove timeline payload keeps the new field

**Files:**
- Modify: `openjax-gateway/src/state/events.rs` tests or `openjax-gateway/tests/gateway_api.rs`

- [ ] **Step 1: Add the lowest-cost persistence/timeline assertion**

Preferred shape:

- map or append a `tool_call_completed` event with `shell_metadata`
- persist via existing state/event path
- assert the stored or replayed payload still includes `shell_metadata`

- [ ] **Step 2: Run the targeted test and verify behavior**

Run the narrowest relevant command for the chosen test location.

Expected: PASS after mapper fix, proving no extra persistence changes are needed.

## Task 5: Update gateway docs and run crate verification

**Files:**
- Modify: `openjax-gateway/README.md`
- Modify: `docs/superpowers/specs/2026-03-28-gateway-tool-call-completed-design.md` if implementation details changed
- Modify: `docs/superpowers/plans/2026-03-28-gateway-tool-call-completed.md` if verification commands changed

- [ ] **Step 1: Update README event contract wording**

Document that gateway `tool_call_completed` now carries `tool_call_id`, `display_name`, and optional `shell_metadata`.

- [ ] **Step 2: Run crate verification**

Run:

- `zsh -lc "cargo test -p openjax-gateway"`
- `zsh -lc "cargo build -p openjax-gateway"`

Expected: PASS

- [ ] **Step 3: Record results**

Capture the exact commands and outcomes in the final report.
