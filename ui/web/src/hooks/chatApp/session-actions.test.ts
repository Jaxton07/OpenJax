import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import type { ChatState } from "../../types/chat";
import type { GatewayClient } from "../../lib/gatewayClient";
import { createBusyTurnNotifier, BUSY_TURN_BLOCKED_MESSAGE } from "./busyTurnNotifier";
import { clearConversationAction, sendMessageAction } from "./session-actions";

function baseState(): ChatState {
  return {
    settings: {
      baseUrl: "http://127.0.0.1:8765",
      outputMode: "sse",
      selectedProviderId: null,
      selectedModelName: null
    },
    auth: { authenticated: true, accessToken: "token", sessionId: "sess_auth", scope: "owner" },
    sessions: [],
    sessionsNextCursor: null,
    sessionsLoadingMore: false,
    activeSessionId: "sess_1",
    globalError: null,
    infoToast: null,
    loading: false
  };
}

describe("busy turn notifier", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-31T00:00:00.000Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("deduplicates blocked-send message within 1500ms", () => {
    const emit = vi.fn();
    const notify = createBusyTurnNotifier(emit);

    expect(notify()).toBe(true);
    expect(notify()).toBe(false);
    expect(emit).toHaveBeenCalledTimes(1);
    expect(emit).toHaveBeenLastCalledWith(BUSY_TURN_BLOCKED_MESSAGE);

    vi.advanceTimersByTime(1500);
    expect(notify()).toBe(true);
    expect(emit).toHaveBeenCalledTimes(2);
  });
});

describe("sendMessageAction", () => {
  it("blocks submitting turn before optimistic insert and submitTurn", async () => {
    const submitTurn = vi.fn();
    const ensureSession = vi.fn().mockResolvedValue("sess_1");
    const updateSession = vi.fn();
    const notifyBusyTurnBlockedSend = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession,
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState: vi.fn(),
      getSessionTurnPhase: () => "submitting",
      notifyBusyTurnBlockedSend
    });

    expect(notifyBusyTurnBlockedSend).toHaveBeenCalledTimes(1);
    expect(updateSession).not.toHaveBeenCalled();
    expect(submitTurn).not.toHaveBeenCalled();
  });

  it("blocks streaming turn before optimistic insert and submitTurn", async () => {
    const submitTurn = vi.fn();
    const ensureSession = vi.fn().mockResolvedValue("sess_1");
    const updateSession = vi.fn();
    const notifyBusyTurnBlockedSend = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession,
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState: vi.fn(),
      getSessionTurnPhase: () => "streaming",
      notifyBusyTurnBlockedSend
    });

    expect(notifyBusyTurnBlockedSend).toHaveBeenCalledTimes(1);
    expect(updateSession).not.toHaveBeenCalled();
    expect(submitTurn).not.toHaveBeenCalled();
  });

  it("uses conflict fallback notifier path", async () => {
    const submitTurn = vi.fn().mockRejectedValue({
      code: "CONFLICT",
      status: 409,
      message: "another turn is still running",
      retryable: false
    });
    const ensureSession = vi.fn().mockResolvedValue("sess_1");
    const updateSession = vi.fn();
    const setState = vi.fn();
    const notifyBusyTurnBlockedSend = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession,
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState,
      getSessionTurnPhase: () => "draft",
      getSessionTitle: () => "new chat",
      getSessionIsPlaceholderTitle: () => true,
      getSessionMessageCount: () => 0,
      notifyBusyTurnBlockedSend
    });

    expect(updateSession).toHaveBeenCalledTimes(2);
    const optimisticUpdater = updateSession.mock.calls[0]?.[1] as ((state: any) => any);
    const rollbackUpdater = updateSession.mock.calls[1]?.[1] as ((state: any) => any);
    const optimisticBase = {
      id: "sess_1",
      title: "test",
      isPlaceholderTitle: true,
      createdAt: "2026-03-31T00:00:00Z",
      connection: "idle",
      turnPhase: "draft",
      lastEventSeq: 3,
      messages: [],
      pendingApprovals: []
    };
    const optimisticSession = optimisticUpdater(optimisticBase);
    expect(optimisticSession.turnPhase).toBe("submitting");
    expect(optimisticSession.title).toBe("hello");
    expect(optimisticSession.messages).toHaveLength(1);
    const rollbackSession = rollbackUpdater(optimisticSession);
    expect(rollbackSession.turnPhase).toBe("draft");
    expect(rollbackSession.title).toBe("new chat");
    expect(rollbackSession.isPlaceholderTitle).toBe(true);
    expect(rollbackSession.messages).toHaveLength(0);
    expect(notifyBusyTurnBlockedSend).toHaveBeenCalledTimes(1);
    expect(setState).not.toHaveBeenCalled();
  });

  it("does not rollback title when concurrent update has replaced optimistic title", async () => {
    const submitTurn = vi.fn().mockRejectedValue({
      code: "CONFLICT",
      status: 409,
      message: "another turn is still running",
      retryable: false
    });
    const updateSession = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession: async () => "sess_1",
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState: vi.fn(),
      getSessionTurnPhase: () => "draft",
      getSessionTitle: () => "new chat",
      getSessionIsPlaceholderTitle: () => true,
      getSessionMessageCount: () => 0,
      notifyBusyTurnBlockedSend: vi.fn()
    });

    const rollbackUpdater = updateSession.mock.calls[1]?.[1] as ((state: any) => any);
    const postConcurrentSession = {
      id: "sess_1",
      title: "远端新标题",
      isPlaceholderTitle: false,
      createdAt: "2026-03-31T00:00:00Z",
      connection: "idle",
      turnPhase: "submitting",
      lastEventSeq: 3,
      messages: [{ id: "optimistic", kind: "text", role: "user", content: "hello" }],
      pendingApprovals: []
    };
    const rolledBack = rollbackUpdater(postConcurrentSession);
    expect(rolledBack.title).toBe("远端新标题");
    expect(rolledBack.isPlaceholderTitle).toBe(false);
  });

  it("removes only optimistic message on conflict and keeps history messages", async () => {
    const submitTurn = vi.fn().mockRejectedValue({
      code: "CONFLICT",
      status: 409,
      message: "another turn is still running",
      retryable: false
    });
    const updateSession = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession: async () => "sess_1",
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState: vi.fn(),
      getSessionTurnPhase: () => "draft",
      getSessionTitle: () => "old title",
      getSessionIsPlaceholderTitle: () => false,
      getSessionMessageCount: () => 1,
      notifyBusyTurnBlockedSend: vi.fn()
    });

    const optimisticUpdater = updateSession.mock.calls[0]?.[1] as ((state: any) => any);
    const rollbackUpdater = updateSession.mock.calls[1]?.[1] as ((state: any) => any);
    const before = {
      id: "sess_1",
      title: "old title",
      isPlaceholderTitle: false,
      createdAt: "2026-03-31T00:00:00Z",
      connection: "idle",
      turnPhase: "draft",
      lastEventSeq: 3,
      messages: [{ id: "history_1", kind: "text", role: "user", content: "history" }],
      pendingApprovals: []
    };
    const optimistic = optimisticUpdater(before);
    const rolledBack = rollbackUpdater(optimistic);
    expect(rolledBack.messages).toHaveLength(1);
    expect(rolledBack.messages[0].id).toBe("history_1");
  });

  it("keeps normal path for non-busy turn", async () => {
    const submitTurn = vi.fn().mockResolvedValue({ turn_id: "turn_1" });
    const ensureSession = vi.fn().mockResolvedValue("sess_1");
    const updateSession = vi.fn();
    const notifyBusyTurnBlockedSend = vi.fn();

    await sendMessageAction({
      content: "hello",
      ensureSession,
      updateSession,
      withAuthRetry: async (action) => action(),
      client: { submitTurn } as unknown as GatewayClient,
      outputMode: "sse",
      pollingAbortRef: { current: null },
      clearAuthState: vi.fn(),
      setState: vi.fn(),
      getSessionTurnPhase: () => "draft",
      notifyBusyTurnBlockedSend
    });

    expect(notifyBusyTurnBlockedSend).not.toHaveBeenCalled();
    expect(updateSession).toHaveBeenCalledTimes(1);
    expect(submitTurn).toHaveBeenCalledTimes(1);
  });

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
    expect(updateSession).toHaveBeenCalledWith("sess_new", expect.any(Function));
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
});

describe("clearConversationAction", () => {
  it("resets cleared session to placeholder title state", async () => {
    const updateSession = vi.fn();
    const setState = vi.fn();
    await clearConversationAction({
      activeSessionId: "sess_1",
      withAuthRetry: async (action) => action(),
      client: { clearSession: vi.fn().mockResolvedValue(undefined) } as unknown as GatewayClient,
      updateSession,
      clearAuthState: vi.fn(),
      setState
    });

    expect(updateSession).toHaveBeenCalledTimes(1);
    const updater = updateSession.mock.calls[0]?.[1] as ((state: any) => any);
    const cleared = updater({
      id: "sess_1",
      title: "old title",
      isPlaceholderTitle: false,
      createdAt: "2026-03-31T00:00:00Z",
      connection: "idle",
      turnPhase: "completed",
      lastEventSeq: 3,
      messages: [{ id: "history_1", kind: "text", role: "user", content: "history" }],
      pendingApprovals: []
    });
    expect(cleared.title).toBe("新聊天");
    expect(cleared.isPlaceholderTitle).toBe(true);
    expect(cleared.turnPhase).toBe("draft");
    expect(cleared.messages).toHaveLength(0);
    expect(setState).toHaveBeenCalled();
  });
});
