# WebUI Session Title + Conflict Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix High+Medium issues for session title recovery, send race correctness, `409` rollback safety, and gateway auto-title atomicity/idempotency without changing public APIs.

**Architecture:** Introduce an explicit session-scoped placeholder-title bit in web state and centralize title resolution in one function reused by all hydration/build paths. Harden send path by capturing target session id and precise rollback contract. Move gateway auto-title condition/update into an atomic store operation and add minimal idempotency guard scoped to auto-title side effects.

**Tech Stack:** React + TypeScript + Vitest (`ui/web`), Rust (`openjax-gateway`), cargo test.

---

### Task 1: Session Title Model Unification (`isPlaceholderTitle` + resolver)

**Files:**
- Modify: `ui/web/src/types/chat.ts`
- Modify: `ui/web/src/hooks/chatApp/session-model.ts`
- Modify: `ui/web/src/hooks/useChatApp.ts`
- Modify: `ui/web/src/hooks/chatApp/session-model.test.ts`
- Modify: `ui/web/src/hooks/useChatApp.hydration.test.ts`

- [ ] **Step 1: Write failing tests for title precedence and placeholder semantics**

Add cases covering:
1. `remote title > local non-placeholder > inferred > placeholder`
2. Placeholder title must be overridable by inferred title.
3. Multi-session isolation for placeholder bit.
4. Inferred title formatting uses `\s+` collapse + trim + code-point-24 truncation + `...`.

- [ ] **Step 2: Run targeted tests to confirm failures**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-model.test.ts src/hooks/useChatApp.hydration.test.ts"`
Expected: FAIL on new assertions.

- [ ] **Step 3: Add `isPlaceholderTitle` field and resolver**

Implement `resolveSessionTitle(...)` and replace truthy merge patterns.

- [ ] **Step 3.1: Normalize inferred-title constants in frontend**

Ensure resolver/summarizer uses a single source of constants for:
1. whitespace normalization (`\s+ -> " "` then trim)
2. truncation limit (`24` code points)
3. overflow suffix (`...`)

- [ ] **Step 4: Update all session constructors/hydration paths**

Ensure every `ChatSession` creation sets `isPlaceholderTitle` explicitly.

- [ ] **Step 5: Re-run targeted tests**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-model.test.ts src/hooks/useChatApp.hydration.test.ts"`
Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add ui/web/src/types/chat.ts ui/web/src/hooks/chatApp/session-model.ts ui/web/src/hooks/useChatApp.ts ui/web/src/hooks/chatApp/session-model.test.ts ui/web/src/hooks/useChatApp.hydration.test.ts
git commit -m "fix(web): unify session title resolution with placeholder state"
```

### Task 2: Same-Tick Session Switch/Send Race Hardening

**Files:**
- Modify: `ui/web/src/hooks/useChatApp.ts`
- Modify: `ui/web/src/hooks/useChatApp.new-chat.test.ts`

- [ ] **Step 1: Add failing race tests**

Add tests:
1. `switchSession("B")` then immediate `sendMessage("x")` routes to `B`.
2. Draft create in flight + user switches back to old session: send targets current active session only.

- [ ] **Step 2: Run race tests to verify failure**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/useChatApp.new-chat.test.ts"`
Expected: FAIL on new race assertions.

- [ ] **Step 3: Implement target-session capture and ref sync**

1. Sync `activeSessionIdRef.current` in `switchSession/newChat` state transitions.
2. Capture target session id at send entry and use throughout submit flow.

- [ ] **Step 4: Re-run race tests**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/useChatApp.new-chat.test.ts"`
Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add ui/web/src/hooks/useChatApp.ts ui/web/src/hooks/useChatApp.new-chat.test.ts
git commit -m "fix(web): prevent stale-session routing in switch-send race"
```

### Task 3: Deterministic `409` Rollback Without Title Pollution

**Files:**
- Modify: `ui/web/src/hooks/chatApp/session-actions.ts`
- Modify: `ui/web/src/hooks/chatApp/session-actions.test.ts`
- Modify: `ui/web/src/hooks/useChatApp.ts`

- [ ] **Step 1: Add failing `409` precision rollback tests**

Add cases:
1. Remove only optimistic message.
2. Restore phase from submitting.
3. First-message title rollback only if current title still equals optimistic title.
4. Preserve concurrent title update.
5. `409` path triggers busy-turn hint and does not set/raise `globalError`.
6. Rollback targets the exact optimistic message by unique `optimisticMessageId` (UUID v4).

- [ ] **Step 2: Run action tests to confirm failure**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts"`
Expected: FAIL for new rollback assertions.

- [ ] **Step 3: Implement rollback guard conditions**

Track `optimisticTitle` + `priorIsPlaceholderTitle`; gate title rollback with current-title equality check.
Keep `optimisticMessageId` as UUID v4 and use it as the only rollback deletion key.
Keep `409` branch behavior explicit: notify busy-turn hint and avoid global error promotion.

- [ ] **Step 4: Re-run action tests**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts"`
Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add ui/web/src/hooks/chatApp/session-actions.ts ui/web/src/hooks/chatApp/session-actions.test.ts ui/web/src/hooks/useChatApp.ts
git commit -m "fix(web): make conflict rollback deterministic and non-destructive"
```

### Task 4: Gateway Auto-Title Atomic CAS (TOCTOU Fix)

**Files:**
- Modify: `openjax-gateway/src/transcript/session_index_store.rs`
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`
- Modify: `openjax-gateway/tests/gateway_api/m9_session_index_store.rs` (if needed for CAS unit coverage)

- [ ] **Step 1: Add failing tests for manual-rename vs auto-title race safety**

Add/adjust tests to assert existing non-empty/manual title cannot be overwritten by first user-message auto naming under concurrent timing.

- [ ] **Step 2: Run gateway suite slice to verify failure**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: FAIL on new race-related assertions.

- [ ] **Step 3: Implement atomic `set-title-if-empty` in session index store**

Add a dedicated method under store write lock and call it from `events.rs` refresh path.

- [ ] **Step 3.1: Align gateway inferred-title formatting constants**

Make sure auto-title derivation keeps the exact frontend-matching semantics:
1. `\s+` collapse + trim
2. code-point-24 truncation
3. `...` suffix

- [ ] **Step 4: Re-run gateway suite slice**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add openjax-gateway/src/transcript/session_index_store.rs openjax-gateway/src/state/events.rs openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs openjax-gateway/tests/gateway_api/m9_session_index_store.rs
git commit -m "fix(gateway): make auto-title update atomic to avoid rename overwrite"
```

### Task 5: Minimal Idempotency for Auto-Title Side Effects

**Files:**
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/src/state/mod.rs` (if state holder extension is needed)
- Modify: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`

- [ ] **Step 1: Add failing duplicate-ingest tests**

Add test that repeats logically same `user_message` ingest key and verifies auto-title side effect executes once.
Add boundary tests:
1. TTL expiry after 30s allows key to be treated as new.
2. Capacity overflow above 10_000 evicts oldest key first.

- [ ] **Step 2: Run focused gateway tests to confirm failure**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: FAIL on new idempotency assertion.

- [ ] **Step 3: Implement recent-window idempotency guard**

1. Key: `(session_id, turn_id, turn_seq, event_type=user_message, normalized_content_hash)`
2. TTL: `30s`
3. Cap: `10_000`, evict oldest on overflow.
4. Scope: guard auto-title side effect only.

- [ ] **Step 3.1: Wire TTL/capacity constants into tests**

Expose constants in test-visible scope (or helper) so boundary tests assert exact configured behavior.

- [ ] **Step 4: Re-run focused gateway tests**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: PASS.

- [ ] **Step 5: Commit Task 5**

```bash
git add openjax-gateway/src/state/events.rs openjax-gateway/src/state/mod.rs openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs
git commit -m "fix(gateway): add scoped idempotency guard for auto-title side effects"
```

### Task 6: Cross-Package Regression Verification

**Files:**
- Modify: none expected (verification only)

- [ ] **Step 1: Run frontend targeted suites**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts src/hooks/useChatApp.new-chat.test.ts src/hooks/useChatApp.hydration.test.ts src/hooks/chatApp/session-model.test.ts"`
Expected: PASS.

- [ ] **Step 2: Run gateway integration suite**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite"`
Expected: PASS.

- [ ] **Step 2.1: Run cross-stack title-format parity assertions**

Run:
1. `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-model.test.ts"`
2. `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`

Expected: PASS on cases asserting same normalize/truncate behavior.

- [ ] **Step 3: Run web build**

Run: `zsh -lc "cd ui/web && pnpm build"`
Expected: PASS.

- [ ] **Step 4: Commit verification note (only if docs evidence file is added)**

```bash
git add <evidence-file-if-any>
git commit -m "test: record verification evidence for session title/conflict repair"
```
