# Draft Policy Level Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在草稿态（`activeSession == null`）支持切换 policy level，并在首条消息发送时自动将所选 policy 应用到新建 session。

**Architecture:** 新增独立 `draftPolicyLevel` state 于 `useChatApp`（不污染 `ChatState`）；提供统一 `onPolicyLevelChange` handler 分流草稿与已有 session 两种路径；`sendMessageAction` 在 `ensureSession()` 之后、发送之前插入 policy 应用步骤，失败则中止。

**Tech Stack:** React (hooks), TypeScript, Vitest + @testing-library/react

---

## File Map

| 文件 | 操作 | 职责变化 |
|------|------|----------|
| `ui/web/src/hooks/chatApp/session-actions.ts` | Modify | `SendMessageParams` 增加 `isDraftSend` / `getDraftPolicyLevel`；`sendMessageAction` 插入 policy 应用逻辑 |
| `ui/web/src/hooks/chatApp/session-actions.test.ts` | Modify | 新增草稿 policy 相关测试 case |
| `ui/web/src/hooks/useChatApp.ts` | Modify | 新增 `draftPolicyLevel` state + ref；新增 `onPolicyLevelChange`；`newChat` 后 reset；`sendMessage` 传 draft policy 参数；return 新增两字段 |
| `ui/web/src/hooks/useChatApp.new-chat.test.ts` | Modify | 扩展 mock 加入 `setPolicyLevel`；新增草稿 policy 集成测试 case |
| `ui/web/src/App.tsx` | Modify | 解构 `draftPolicyLevel` 和 `onPolicyLevelChange`；移除 `sendPolicyLevel`；修复 `policyLevel` prop 和 `onPolicyLevelChange` prop |

---

## Task 1: 扩展 `sendMessageAction` 支持草稿 policy

**Files:**
- Modify: `ui/web/src/hooks/chatApp/session-actions.ts`
- Test: `ui/web/src/hooks/chatApp/session-actions.test.ts`

- [ ] **Step 1: 为草稿 policy 写失败测试**

在 `session-actions.test.ts` 的 `describe("sendMessageAction", ...)` 块末尾追加以下 4 个测试：

```ts
it("calls setPolicyLevel before submitTurn when isDraftSend=true and draftPolicyLevel=allow", async () => {
  const callOrder: string[] = [];
  const setPolicyLevel = vi.fn().mockImplementation(async () => {
    callOrder.push("setPolicyLevel");
  });
  const submitTurn = vi.fn().mockImplementation(async () => {
    callOrder.push("submitTurn");
    return { turn_id: "turn_1" };
  });
  const updateSession = vi.fn();

  await sendMessageAction({
    content: "hello",
    ensureSession: vi.fn().mockResolvedValue("sess_new"),
    updateSession,
    withAuthRetry: async (action) => action(),
    client: { submitTurn, setPolicyLevel } as unknown as GatewayClient,
    outputMode: "sse",
    pollingAbortRef: { current: null },
    clearAuthState: vi.fn(),
    setState: vi.fn(),
    getSessionTurnPhase: () => "draft",
    isDraftSend: true,
    getDraftPolicyLevel: () => "allow",
  });

  expect(callOrder).toEqual(["setPolicyLevel", "submitTurn"]);
  expect(setPolicyLevel).toHaveBeenCalledWith("sess_new", "allow");
  expect(updateSession).toHaveBeenCalledWith(
    "sess_new",
    expect.any(Function)
  );
  // Verify updateSession sets policyLevel
  const updater = updateSession.mock.calls[0]?.[1] as (s: any) => any;
  const updated = updater({ id: "sess_new", messages: [], pendingApprovals: [] });
  expect(updated.policyLevel).toBe("allow");
});

it("skips setPolicyLevel when draftPolicyLevel=ask even if isDraftSend=true", async () => {
  const setPolicyLevel = vi.fn();
  const submitTurn = vi.fn().mockResolvedValue({ turn_id: "turn_1" });

  await sendMessageAction({
    content: "hello",
    ensureSession: vi.fn().mockResolvedValue("sess_new"),
    updateSession: vi.fn(),
    withAuthRetry: async (action) => action(),
    client: { submitTurn, setPolicyLevel } as unknown as GatewayClient,
    outputMode: "sse",
    pollingAbortRef: { current: null },
    clearAuthState: vi.fn(),
    setState: vi.fn(),
    getSessionTurnPhase: () => "draft",
    isDraftSend: true,
    getDraftPolicyLevel: () => "ask",
  });

  expect(setPolicyLevel).not.toHaveBeenCalled();
  expect(submitTurn).toHaveBeenCalledTimes(1);
});

it("aborts send and sets globalError when setPolicyLevel fails in draft send", async () => {
  const setPolicyLevel = vi.fn().mockRejectedValue(new Error("network error"));
  const submitTurn = vi.fn().mockResolvedValue({ turn_id: "turn_1" });
  const setState = vi.fn();

  await sendMessageAction({
    content: "hello",
    ensureSession: vi.fn().mockResolvedValue("sess_new"),
    updateSession: vi.fn(),
    withAuthRetry: async (action) => action(),
    client: { submitTurn, setPolicyLevel } as unknown as GatewayClient,
    outputMode: "sse",
    pollingAbortRef: { current: null },
    clearAuthState: vi.fn(),
    setState,
    getSessionTurnPhase: () => "draft",
    isDraftSend: true,
    getDraftPolicyLevel: () => "allow",
  });

  expect(submitTurn).not.toHaveBeenCalled();
  // setState called with a function that sets globalError
  const lastCall = setState.mock.calls[setState.mock.calls.length - 1]?.[0] as (s: any) => any;
  const next = lastCall({ globalError: null });
  expect(next.globalError).toBeTruthy();
});

it("aborts send and calls clearAuthState when setPolicyLevel returns auth error", async () => {
  const setPolicyLevel = vi.fn().mockRejectedValue({
    code: "UNAUTHENTICATED",
    status: 401,
    message: "token expired"
  });
  const submitTurn = vi.fn();
  const clearAuthState = vi.fn();

  await sendMessageAction({
    content: "hello",
    ensureSession: vi.fn().mockResolvedValue("sess_new"),
    updateSession: vi.fn(),
    withAuthRetry: async (action) => action(),
    client: { submitTurn, setPolicyLevel } as unknown as GatewayClient,
    outputMode: "sse",
    pollingAbortRef: { current: null },
    clearAuthState,
    setState: vi.fn(),
    getSessionTurnPhase: () => "draft",
    isDraftSend: true,
    getDraftPolicyLevel: () => "allow",
  });

  expect(clearAuthState).toHaveBeenCalledTimes(1);
  expect(submitTurn).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: 运行新测试确认它们失败**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test -- session-actions --run 2>&1 | tail -30"
```

预期：4 个新测试报 `TypeError` 或 `AssertionError`（`isDraftSend` 参数未被识别）。

- [ ] **Step 3: 在 `session-actions.ts` 中扩展 `SendMessageParams` 并实现逻辑**

在 `SendMessageParams` 接口末尾加两个可选字段（`notifyBusyTurnBlockedSend?` 之后）：

```ts
  isDraftSend?: boolean;
  getDraftPolicyLevel?: () => "allow" | "ask" | "deny";
```

在 `sendMessageAction` 函数体中，找到 `const sessionId = await params.ensureSession();` 这一行，在其**正下方**（`const gateAccepted =` 之前）插入：

```ts
  // Apply draft policy for newly created sessions before gating
  if (params.isDraftSend && params.getDraftPolicyLevel) {
    const level = params.getDraftPolicyLevel();
    if (level !== "ask") {
      try {
        await params.withAuthRetry(() => params.client.setPolicyLevel(sessionId, level));
        params.updateSession(sessionId, (s) => ({ ...s, policyLevel: level }));
      } catch (error) {
        if (isAuthenticationError(error)) {
          params.clearAuthState("登录态已失效，请重新登录。");
          return;
        }
        params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
        return;
      }
    }
  }
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test -- session-actions --run 2>&1 | tail -20"
```

预期：所有测试（含旧有）全部 PASS。

- [ ] **Step 5: Commit**

```bash
cd /Users/ericw/work/code/ai/openJax
git add ui/web/src/hooks/chatApp/session-actions.ts \
        ui/web/src/hooks/chatApp/session-actions.test.ts
git commit -m "feat(web): sendMessageAction 支持草稿 policy 首发前应用

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 2: 在 `useChatApp` 中增加草稿 policy 状态与统一 handler

**Files:**
- Modify: `ui/web/src/hooks/useChatApp.ts`
- Test: `ui/web/src/hooks/useChatApp.new-chat.test.ts`

- [ ] **Step 1: 扩展 mock，写失败测试**

在 `useChatApp.new-chat.test.ts` 中：

**1a. 在 `mocks` 对象中加入 `setPolicyLevel`**（找到 `vi.hoisted` 块）：

```ts
const mocks = vi.hoisted(() => {
  const startSession = vi.fn();
  const submitTurn = vi.fn();
  const setPolicyLevel = vi.fn();
  return { startSession, submitTurn, setPolicyLevel };
});

vi.mock("../lib/gatewayClient", () => ({
  GatewayClient: vi.fn().mockImplementation(() => ({
    startSession: mocks.startSession,
    submitTurn: mocks.submitTurn,
    setPolicyLevel: mocks.setPolicyLevel
  }))
}));
```

**1b. 在现有 `beforeEach` 中加一行 reset**（在 `mocks.submitTurn.mockReset()...` 之后）：

```ts
mocks.setPolicyLevel.mockReset().mockResolvedValue({ level: "allow" });
```

**1c. 在 `describe("useChatApp newChat guard", ...)` 块末尾追加 4 个测试**：

```ts
it("onPolicyLevelChange in draft mode updates draftPolicyLevel without calling setPolicyLevel API", async () => {
  let apiRef: ReturnType<typeof useChatApp> | null = null;
  render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
  await waitFor(() => expect(apiRef).not.toBeNull());

  expect(apiRef!.state.activeSessionId).toBeNull();

  await act(async () => {
    apiRef!.onPolicyLevelChange("allow");
  });

  expect(apiRef!.draftPolicyLevel).toBe("allow");
  expect(mocks.setPolicyLevel).not.toHaveBeenCalled();
});

it("newChat resets draftPolicyLevel to ask", async () => {
  saveSessionsForBoot([
    {
      id: "sess_used",
      title: "老会话",
      isPlaceholderTitle: false,
      createdAt: "2026-01-01T00:00:00Z",
      connection: "idle",
      turnPhase: "completed",
      lastEventSeq: 2,
      messages: [
        { id: "msg_1", kind: "text", role: "user", content: "hello", timestamp: "2026-01-01T00:00:01Z" }
      ],
      pendingApprovals: []
    }
  ]);

  let apiRef: ReturnType<typeof useChatApp> | null = null;
  render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
  await waitFor(() => expect(apiRef).not.toBeNull());

  // Go to draft and set policy to allow
  await act(async () => { await apiRef!.newChat(); });
  await act(async () => { apiRef!.onPolicyLevelChange("allow"); });
  expect(apiRef!.draftPolicyLevel).toBe("allow");

  // Switch back to existing session then new chat again
  await act(async () => { apiRef!.switchSession("sess_used"); });
  await act(async () => { await apiRef!.newChat(); });

  expect(apiRef!.draftPolicyLevel).toBe("ask");
});

it("draft send with draftPolicyLevel=allow calls startSession → setPolicyLevel → submitTurn in order", async () => {
  const callOrder: string[] = [];
  mocks.startSession.mockReset().mockImplementation(async () => {
    callOrder.push("startSession");
    return { request_id: "req_1", session_id: "sess_new_1", timestamp: "2026-01-01T00:00:00Z" };
  });
  mocks.setPolicyLevel.mockReset().mockImplementation(async () => {
    callOrder.push("setPolicyLevel");
    return { level: "allow" };
  });
  mocks.submitTurn.mockReset().mockImplementation(async () => {
    callOrder.push("submitTurn");
    return { request_id: "req_t1", session_id: "sess_new_1", turn_id: "turn_1", timestamp: "2026-01-01T00:00:01Z" };
  });

  let apiRef: ReturnType<typeof useChatApp> | null = null;
  render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
  await waitFor(() => expect(apiRef).not.toBeNull());

  // Set draft policy to allow
  await act(async () => { apiRef!.onPolicyLevelChange("allow"); });
  expect(apiRef!.draftPolicyLevel).toBe("allow");

  // Send first message
  await act(async () => { await apiRef!.sendMessage("hello"); });

  expect(callOrder).toEqual(["startSession", "setPolicyLevel", "submitTurn"]);
  expect(mocks.setPolicyLevel).toHaveBeenCalledWith("sess_new_1", "allow");
});

it("draft send aborts and sets globalError when setPolicyLevel fails", async () => {
  mocks.startSession.mockReset().mockResolvedValue({
    request_id: "req_1",
    session_id: "sess_new_1",
    timestamp: "2026-01-01T00:00:00Z"
  });
  mocks.setPolicyLevel.mockReset().mockRejectedValue(new Error("network error"));

  let apiRef: ReturnType<typeof useChatApp> | null = null;
  render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
  await waitFor(() => expect(apiRef).not.toBeNull());

  await act(async () => { apiRef!.onPolicyLevelChange("allow"); });
  await act(async () => { await apiRef!.sendMessage("hello"); });

  expect(mocks.submitTurn).not.toHaveBeenCalled();
  expect(apiRef!.state.globalError).toBeTruthy();
});
```

- [ ] **Step 2: 运行新测试确认失败**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test -- useChatApp.new-chat --run 2>&1 | tail -30"
```

预期：4 个新测试报 `TypeError`（`onPolicyLevelChange` / `draftPolicyLevel` 未在 hook 返回值中）。

- [ ] **Step 3: 在 `useChatApp.ts` 中实现**

**3a. 在 `useChatApp` 函数体顶部（现有 state 声明之后，`reconnectAbortRef` 之前）加入**：

```ts
  const [draftPolicyLevel, setDraftPolicyLevel] = useState<"allow" | "ask" | "deny">("ask");
  const draftPolicyLevelRef = useRef<"allow" | "ask" | "deny">("ask");
  useEffect(() => {
    draftPolicyLevelRef.current = draftPolicyLevel;
  }, [draftPolicyLevel]);
```

**3b. 修改 `newChat` callback**，在 `await newChatAction(...)` 之后加一行：

```ts
  const newChat = useCallback(async () => {
    if (activeSession && !isEmptyDraftSession(activeSession)) {
      activeSessionIdRef.current = null;
    }
    await newChatAction({
      activeSession,
      withAuthRetry,
      client,
      setState,
      clearAuthState
    });
    setDraftPolicyLevel("ask");    // ← 新增：每次进入草稿态重置
  }, [activeSession, clearAuthState, client, withAuthRetry]);
```

**3c. 修改 `sendMessage` callback**，在 `sendMessageAction` 调用的参数对象中新增两字段（紧接 `ensureSession` 之后）：

```ts
      isDraftSend: !targetSessionId,
      getDraftPolicyLevel: () => draftPolicyLevelRef.current,
```

完整 sendMessage 参数列表如下（展示新增行位置）：

```ts
    await sendMessageAction({
      content,
      ensureSession: () => ensureSession(targetSessionId),
      isDraftSend: !targetSessionId,                          // ← 新增
      getDraftPolicyLevel: () => draftPolicyLevelRef.current, // ← 新增
      updateSession,
      withAuthRetry,
      client,
      outputMode: state.settings.outputMode,
      pollingAbortRef,
      clearAuthState,
      setState,
      getSessionTurnPhase: (sessionId: string) =>
        sessionsRef.current.find((session) => session.id === sessionId)?.turnPhase,
      getSessionTitle: (sessionId: string) =>
        sessionsRef.current.find((session) => session.id === sessionId)?.title,
      getSessionIsPlaceholderTitle: (sessionId: string) =>
        sessionsRef.current.find((session) => session.id === sessionId)?.isPlaceholderTitle,
      getSessionMessageCount: (sessionId: string) =>
        sessionsRef.current.find((session) => session.id === sessionId)?.messages.length,
      tryBeginSubmit: (sessionId: string) => {
        if (submittingSessionIdsRef.current.has(sessionId)) {
          return false;
        }
        submittingSessionIdsRef.current.add(sessionId);
        return true;
      },
      endSubmit: (sessionId: string) => {
        submittingSessionIdsRef.current.delete(sessionId);
      },
      notifyBusyTurnBlockedSend
    });
```

**3d. 在 `sendPolicyLevel` 之后新增 `onPolicyLevelChange` callback**：

```ts
  const onPolicyLevelChange = useCallback(
    (level: "allow" | "ask" | "deny") => {
      if (activeSession != null) {
        void sendPolicyLevel(activeSession.id, level);
      } else {
        setDraftPolicyLevel(level);
      }
    },
    [activeSession, sendPolicyLevel]
  );
```

**3e. 在 `return` 对象中加入两个新字段**（放在 `sendPolicyLevel` 旁边）：

```ts
    draftPolicyLevel,
    onPolicyLevelChange,
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test -- useChatApp.new-chat --run 2>&1 | tail -20"
```

预期：全部 PASS（含旧有 6 个测试）。

- [ ] **Step 5: 跑全量 web 测试确保无回归**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test --run 2>&1 | tail -20"
```

预期：全部 PASS。

- [ ] **Step 6: Commit**

```bash
cd /Users/ericw/work/code/ai/openJax
git add ui/web/src/hooks/useChatApp.ts \
        ui/web/src/hooks/useChatApp.new-chat.test.ts
git commit -m "feat(web): useChatApp 增加 draftPolicyLevel 状态与 onPolicyLevelChange handler

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3: 修复 App.tsx 使用新 hook API

**Files:**
- Modify: `ui/web/src/App.tsx`

- [ ] **Step 1: 更新 App.tsx 解构与 Composer props**

在 `useChatApp()` 解构中：
- 新增 `draftPolicyLevel`
- 新增 `onPolicyLevelChange`
- 移除 `sendPolicyLevel`（App.tsx 不再直接使用）

完整解构替换为：

```ts
  const {
    state,
    activeSession,
    isAuthenticated,
    authenticate,
    logout,
    newChat,
    loadMoreSessions,
    sidebarHasMore,
    sidebarLoadingMore,
    switchSession,
    deleteSession,
    sendMessage,
    resolveApproval,
    updateSettings,
    testConnection,
    listAuthSessions,
    revokeAuthSession,
    revokeAllAuthSessions,
    listProviders,
    createProvider,
    updateProvider,
    deleteProvider,
    getActiveProvider,
    setActiveProvider,
    fetchCatalog,
    dismissGlobalError,
    dismissToast,
    draftPolicyLevel,
    onPolicyLevelChange,
    isStreaming,
    isBusyTurn,
    abortTurn,
    clearConversation,
    notifyBusyTurnBlockedSend
  } = useChatApp();
```

修改 `<Composer>` 的两个 props：

```tsx
          policyLevel={activeSession?.policyLevel ?? draftPolicyLevel}
          onPolicyLevelChange={onPolicyLevelChange}
```

（原来是 `policyLevel={activeSession?.policyLevel ?? "ask"}` 和 `onPolicyLevelChange={(level) => void sendPolicyLevel(activeSession!.id, level)}`）

- [ ] **Step 2: 运行全量 web 测试**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm test --run 2>&1 | tail -20"
```

预期：全部 PASS。

- [ ] **Step 3: 构建确认无 TS 编译错误**

```bash
cd /Users/ericw/work/code/ai/openJax/ui/web
zsh -lc "pnpm build 2>&1 | tail -20"
```

预期：Build 成功，0 errors。

- [ ] **Step 4: Commit**

```bash
cd /Users/ericw/work/code/ai/openJax
git add ui/web/src/App.tsx
git commit -m "fix(web): App.tsx 使用 onPolicyLevelChange 修复草稿态 policy 报错

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## 验收检查（手动）

完成所有 task 后在本地执行以下验证：

1. `make run-web-dev` 启动，访问 `http://127.0.0.1:5173`
2. 登录后点击「新建对话」进入草稿态
3. 点击 PolicyLevelButton → 切换为 `allow` → 无报错，按钮显示 `allow`
4. 输入消息发送 → 控制台无报错，首轮工具调用不卡审批
5. 切换到历史会话 → policy 展示该会话自身的值，与草稿态不串
6. 返回新建对话 → policy 重置显示 `ask`
