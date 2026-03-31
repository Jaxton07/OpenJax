# WebUI Busy-Turn Send Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent message submission while a turn is `submitting/streaming` (while still allowing typing), show a single deduped user-facing toast, and correct `CONFLICT` messaging.

**Architecture:** Add a two-layer gate. Layer 1 is UI interception in Composer (`Enter` and send button) with a shared callback. Layer 2 is a hard guard in `sendMessageAction` before optimistic user-message insertion and before `submitTurn`, reusing the same notifier. Centralize dedup + message text in `useChatApp` to keep behavior consistent across UI and fallback error paths.

**Tech Stack:** React + TypeScript + Vitest + Testing Library (`ui/web`).

---

### Task 1: Centralize Busy-Turn Notification and Gate State

**Files:**
- Modify: `ui/web/src/hooks/useChatApp.ts`
- Modify: `ui/web/src/App.tsx`
- Modify: `ui/web/src/types/chat.ts` (only if new helper type is needed)
- Test: `ui/web/src/hooks/chatApp/session-actions.test.ts` (created in Task 3)

- [ ] **Step 1: Write a failing dedup test for the shared notifier**

Create test in `ui/web/src/hooks/chatApp/session-actions.test.ts` (or a dedicated hook test) that asserts:
1. two blocked-send notifications inside 1500ms emit one toast update;
2. after advancing timer beyond 1500ms, next blocked-send emits again.

- [ ] **Step 2: Implement `isBusyTurn` and a deduped notifier in `useChatApp`**

```ts
const BUSY_TURN_TOAST = "Please wait for the current response to finish.";
const BUSY_TURN_TOAST_DEDUP_MS = 1500;
const lastBusyTurnToastAtRef = useRef(0);

const isBusyTurn = useMemo(
  () => activeSession?.turnPhase === "submitting" || activeSession?.turnPhase === "streaming",
  [activeSession]
);

const notifyBusyTurnBlockedSend = useCallback(() => {
  const now = Date.now();
  if (now - lastBusyTurnToastAtRef.current < BUSY_TURN_TOAST_DEDUP_MS) return;
  lastBusyTurnToastAtRef.current = now;
  setState((prev) => ({ ...prev, infoToast: BUSY_TURN_TOAST }));
}, []);
```

- [ ] **Step 3: Wire `isBusyTurn` + notifier through `App.tsx` into `Composer` props**

```tsx
<Composer
  ...
  isBusyTurn={isBusyTurn}
  onBlockedSendAttempt={notifyBusyTurnBlockedSend}
/>
```

- [ ] **Step 4: Run focused typecheck/build validation**

Run: `zsh -lc "cd ui/web && pnpm build"`  
Expected: build succeeds, no TS errors.

- [ ] **Step 5: Run dedup test after notifier implementation**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts"`  
Expected: dedup test passes and validates the 1500ms window behavior.

- [ ] **Step 6: Commit Task 1**

```bash
git add ui/web/src/hooks/useChatApp.ts ui/web/src/App.tsx
git commit -m "feat(web): add centralized busy-turn notifier and gate state"
```

### Task 2: Enforce UI-Level Send Interception in Composer

**Files:**
- Modify: `ui/web/src/components/composer/index.tsx`
- Modify: `ui/web/src/components/composer/ComposerInput.tsx`
- Modify: `ui/web/src/components/composer/index.test.tsx`

- [ ] **Step 1: Write failing Composer tests for busy-turn interception**

Add tests for:
- `isBusyTurn=true` + `Enter` => `onSend` not called, `onBlockedSendAttempt` called once
- `isBusyTurn=true` + send button click => `onSend` not called, `onBlockedSendAttempt` called
- `isBusyTurn=true` + typing still updates textarea value

- [ ] **Step 2: Run test file to verify failure**

Run: `zsh -lc "cd ui/web && pnpm test -- src/components/composer/index.test.tsx"`  
Expected: new tests fail before implementation.

- [ ] **Step 3: Implement Composer interception logic**

```ts
const canSubmit = !disabled && !isBusyTurn && input.trim().length > 0;

const attemptSubmit = async () => {
  if (isBusyTurn) {
    onBlockedSendAttempt?.();
    return;
  }
  await submit();
};
```

In `ComposerInput` keydown:

```ts
if (event.key === "Enter" && !event.shiftKey && !disabled) {
  event.preventDefault();
  onSubmit(); // onSubmit now performs busy-turn interception
}
```

Send button behavior:
- Do not use native `disabled` for busy state.
- Keep it clickable; route to `onSubmit` and let interception show toast.

- [ ] **Step 4: Re-run Composer tests**

Run: `zsh -lc "cd ui/web && pnpm test -- src/components/composer/index.test.tsx"`  
Expected: pass.

- [ ] **Step 5: Commit Task 2**

```bash
git add ui/web/src/components/composer/index.tsx ui/web/src/components/composer/ComposerInput.tsx ui/web/src/components/composer/index.test.tsx
git commit -m "feat(web): intercept busy-turn send attempts in composer"
```

### Task 3: Add Business-Layer Hard Guard Before Optimistic Insert

**Files:**
- Modify: `ui/web/src/hooks/chatApp/session-actions.ts`
- Create: `ui/web/src/hooks/chatApp/session-actions.test.ts`
- Modify: `ui/web/src/hooks/useChatApp.ts` (pass notifier + session snapshot/accessor)

- [ ] **Step 1: Write failing unit tests for `sendMessageAction` busy guard**

Test cases:
1. busy session (`submitting`) -> `submitTurn` not called, `updateSession` optimistic insert not called, `notifyBusyTurnBlockedSend` called.
2. busy session (`streaming`) -> same expectations.
3. non-busy session -> existing flow still calls `submitTurn` and optimistic insert.
4. `submitTurn` throws `CONFLICT` -> `notifyBusyTurnBlockedSend` called, and busy-turn message path is used (not login/global-error path).

- [ ] **Step 2: Run unit tests to verify failure**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts"`  
Expected: failing busy-guard tests.

- [ ] **Step 3: Implement minimal hard guard**

```ts
if (params.getSessionTurnPhase?.(sessionId) === "submitting" || params.getSessionTurnPhase?.(sessionId) === "streaming") {
  params.notifyBusyTurnBlockedSend?.();
  return;
}
```

Guard must run before:
- optimistic `messages.push(user)`
- `client.submitTurn(...)`

Also update catch-path handling:

```ts
if (isGatewayConflictError(error)) {
  params.notifyBusyTurnBlockedSend?.();
  return;
}
```

Do not set `globalError` for this specific conflict fallback path.

- [ ] **Step 4: Run new unit tests and targeted regression tests**

Run:
- `zsh -lc "cd ui/web && pnpm test -- src/hooks/chatApp/session-actions.test.ts"`
- `zsh -lc "cd ui/web && pnpm test -- src/components/composer/index.test.tsx"`

Expected: all pass.

- [ ] **Step 5: Commit Task 3**

```bash
git add ui/web/src/hooks/chatApp/session-actions.ts ui/web/src/hooks/chatApp/session-actions.test.ts ui/web/src/hooks/useChatApp.ts
git commit -m "fix(web): block busy-turn sends before optimistic message insert"
```

### Task 4: Correct `CONFLICT` Humanized Message and Add Coverage

**Files:**
- Modify: `ui/web/src/lib/errors.ts`
- Modify: `ui/web/src/lib/errors.test.ts`

- [ ] **Step 1: Add failing test for `CONFLICT` mapping**

```ts
it("maps conflict to busy-turn guidance", () => {
  expect(humanizeError({ code: "CONFLICT", message: "another turn", status: 409, retryable: false }))
    .toBe("Please wait for the current response to finish.");
});
```

- [ ] **Step 2: Run error tests to confirm failure**

Run: `zsh -lc "cd ui/web && pnpm test -- src/lib/errors.test.ts"`  
Expected: new conflict-mapping test fails.

- [ ] **Step 3: Implement message mapping change**

```ts
case "CONFLICT":
  return "Please wait for the current response to finish.";
```

Important: this is fallback copy consistency only. Primary UX for blocked busy-send remains the shared notifier path from Task 1/Task 3.

- [ ] **Step 4: Re-run error tests**

Run: `zsh -lc "cd ui/web && pnpm test -- src/lib/errors.test.ts"`  
Expected: pass.

- [ ] **Step 5: Commit Task 4**

```bash
git add ui/web/src/lib/errors.ts ui/web/src/lib/errors.test.ts
git commit -m "fix(web): map conflict errors to busy-turn user guidance"
```

### Task 5: End-to-End Regression Sweep for Web UI Package

**Files:**
- Modify: none expected (verification only)
- Test: existing `ui/web` test suite

- [ ] **Step 1: Run merged targeted suite**

Run:
`zsh -lc "cd ui/web && pnpm test -- src/components/composer/index.test.tsx src/hooks/chatApp/session-actions.test.ts src/lib/errors.test.ts"`

Expected: all targeted tests pass.

Must include explicit pass evidence for:
1. toast dedup window (1500ms);
2. unified notifier usage across UI interception, busy-guard, and conflict fallback.

- [ ] **Step 2: Run full web test suite**

Run: `zsh -lc "cd ui/web && pnpm test"`  
Expected: pass, no regressions in unrelated modules.

- [ ] **Step 3: Run web build**

Run: `zsh -lc "cd ui/web && pnpm build"`  
Expected: successful production build.

- [ ] **Step 4: Commit verification notes (if a docs/test evidence file is used)**

```bash
# If no files changed, skip commit for this task.
git status --short
```

- [ ] **Step 5: Final handoff summary**

Include:
- changed files list
- test commands + result
- behavior checklist against spec DoD

---

## Implementation Notes

- Keep logic DRY: one toast text constant, one dedup notifier entrypoint.
- Do not modify gateway runtime/orchestrator in this scope.
- Prefer fake timers (`vi.useFakeTimers()`) in dedup tests for deterministic window assertions.
