# 2026-03-31 WebUI Session Title + Conflict Repair Design

## 1. Background

This design addresses confirmed `High + Medium` issues in the current WebUI/gateway behavior:

1. Frontend can route a send to the wrong session in same-tick switch/send race windows.
2. Session title hydration can lock placeholder title and fail to recover real title after restart.
3. Gateway first-user-message auto title naming has TOCTOU risk and can overwrite manual rename.
4. `409 CONFLICT` rollback can pollute session state (`turnPhase`/title) in edge races.
5. Coverage gaps leave critical regressions (hydration title recovery/session isolation) unguarded.

This round intentionally includes minimal idempotency only for the auto-title path in gateway.

## 2. Goals and Non-Goals

### 2.1 Goals

1. Keep title state strictly session-scoped and prevent cross-session pollution.
2. Unify title resolution with one explicit rule used by all frontend hydration paths.
3. Make `409` rollback deterministic and non-destructive to concurrent legitimate updates.
4. Remove gateway auto-title TOCTOU by moving condition check + update into an atomic store operation.
5. Add minimal idempotency guard for auto-title side effects under duplicated `user_message` ingest.
6. Close test gaps for race and restart recovery paths.

### 2.2 Non-Goals

1. No HTTP API shape changes.
2. No global transcript append idempotency redesign in this round.
3. No queue-based send scheduling.
4. No broad refactor unrelated to these defects.

## 3. Chosen Approach

## 3.1 Alternatives Considered

1. Placeholder by literal (`"新聊天"`): smallest patch but fragile.
2. Placeholder by multi-locale string list: still text-coupled.
3. Placeholder by explicit state bit (`isPlaceholderTitle`): clean semantics, selected.

## 3.2 Decision

Use explicit `isPlaceholderTitle` bound to each `ChatSession`.  
Consolidate title merge logic into a single resolver function and apply it across all hydration/build paths.

## 4. Detailed Design

## 4.1 Frontend Session Model and Invariants

Add `isPlaceholderTitle: boolean` to `ChatSession`.

Invariants:

1. `title` + `isPlaceholderTitle` are always updated by `sessionId`-scoped updater only.
2. `switchSession` changes `activeSessionId` only; it must not mutate other session title state.
3. Placeholder fallback always sets `isPlaceholderTitle=true`.
4. Any real title (remote persisted or inferred first user message) sets `isPlaceholderTitle=false`.

Initialization rules:

1. Local draft creation: `title="新聊天"`, `isPlaceholderTitle=true`.
2. Remote summary with non-empty title: `isPlaceholderTitle=false`.
3. Clear/reset to empty draft: `title="新聊天"`, `isPlaceholderTitle=true`.

## 4.2 Unified Title Resolver

Introduce `resolveSessionTitle(...)` and use it in:

1. `buildSidebarSessionsFromSummaries`
2. `buildChatSessionFromGateway`
3. timeline hydration merge in `useChatApp`

Deterministic precedence:

1. `remote persisted title` (non-empty)
2. `local title` when `local.isPlaceholderTitle === false`
3. `inferred title` from first user message (normalized + truncated)
4. placeholder fallback (`"新聊天"`, `isPlaceholderTitle=true`)

This removes all `truthy` merge patterns such as `current.title || rebuilt.title`.

Normalization/truncation source of truth for inferred titles:

1. normalize: collapse whitespace with regex-equivalent `\s+ -> " "` then trim
2. truncate: code-point based `24` chars + `...` suffix when overflow
3. frontend and gateway must use the same constants/semantics

## 4.3 Send Path Race Control

For `switchSession` and `newChat`, synchronously update `activeSessionIdRef.current` where active id changes.  
`sendMessage` captures target `sessionId` at entry and uses that id through ensure/gate/submit/rollback.

This guarantees same-tick switch/send does not route to stale active session.

## 4.4 `409 CONFLICT` Rollback Contract

On optimistic submit, snapshot:

1. `priorTurnPhase`
2. `priorTitle`
3. `priorIsPlaceholderTitle`
4. `priorMessageCount`
5. `optimisticTitle` (if modified by this submit)
6. `optimisticMessageId` generated as UUID v4 (must be unique within client runtime)

On `409`:

1. remove only this submit's optimistic message id
2. restore `turnPhase` only if current phase still reflects this optimistic submit (`submitting`)
3. restore title only when all are true:
   - this was first-message naming path (`priorMessageCount === 0`)
   - current title still equals `optimisticTitle`
4. otherwise keep current title (protect concurrent valid updates)
5. show busy-turn user hint, no global error promotion

## 4.5 Gateway Auto-Title Atomicity (TOCTOU fix)

Move `title empty? then set auto title` into `SessionIndexStore` atomic operation under store write lock:

1. read current entry title
2. if empty and candidate title present -> apply upsert + metadata update
3. if non-empty -> no-op

`events.rs` no longer decides with pre-read `list_sessions()` then update.

## 4.6 Minimal Idempotency for Auto-Title Path

Scope-limited idempotency for repeated `user_message` ingestion:

1. key shape: `(session_id, turn_id, turn_seq, event_type=user_message, normalized_content_hash)`
2. recent-window cache in gateway state layer
3. duplicate key short-circuits auto-title side effect path only

Recent-window guardrail:

1. keep recent keys in memory with fixed TTL window (default 30s)
2. cap map size (default 10_000); evict oldest keys when over capacity
3. these defaults are implementation constants and covered by boundary tests

Transcript append semantics remain unchanged in this round.

## 5. Error Handling

1. `409`: rollback optimistic side effects + busy hint; no auth/global fallback.
2. Auth errors: keep current auth-clearing flow.
3. Non-409 provider errors: keep current global error flow.
4. Timeline hydration failure: keep existing non-blocking behavior; do not overwrite valid local non-placeholder title with placeholder state.

## 6. Test Plan

## 6.1 Frontend

1. `useChatApp.new-chat.test.ts`
   - same-tick `switchSession(B)` then `sendMessage` must target `B`
   - draft create pending + switch back to old session send does not cross-wire
2. `session-actions.test.ts`
   - `409` removes only optimistic message
   - `409` recovers phase from `submitting`
   - first-message rollback restores title only when current still optimistic title
   - concurrent title update before rollback is preserved
3. `useChatApp.hydration.test.ts`
   - summary missing title + timeline first user message -> not placeholder final title
   - persisted remote title always wins
   - session A title state updates never affect session B
4. `session-model.test.ts`
   - resolver precedence table and placeholder flag transitions

## 6.2 Gateway

1. auto-title first message writes title when empty
2. auto-title does not overwrite existing/manual title
3. manual rename concurrent with auto-title does not get overwritten (TOCTOU regression)
4. duplicated user_message on idempotency key does not repeat auto-title side effect

## 7. Acceptance Criteria (DoD)

1. Restart gateway + refresh UI does not collapse historical titled sessions to placeholder.
2. New chat without sending does not persist empty remote session.
3. Busy/conflict path does not leave session stuck in `submitting`.
4. Title state is session-bound and isolated across session switches.
5. Public HTTP API remains unchanged.

## 8. Risks and Mitigations

1. **Risk:** session model field rollout misses one construction path.
   - **Mitigation:** add compile-time required field and update all model constructors/tests.
2. **Risk:** partial ref sync still leaves race windows.
   - **Mitigation:** enforce target `sessionId` capture in `sendMessage` and session-scoped tests.
3. **Risk:** limited idempotency scope may not cover broader replay classes.
   - **Mitigation:** explicitly constrain scope in this design and leave global idempotency to separate spec.
