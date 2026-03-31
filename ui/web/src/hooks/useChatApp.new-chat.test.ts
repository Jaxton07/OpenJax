import { act, render, waitFor } from "@testing-library/react";
import { createElement, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChatApp } from "./useChatApp";
import { BUSY_TURN_BLOCKED_MESSAGE } from "./chatApp/busyTurnNotifier";

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

function HookHarness(props: { onReady: (api: ReturnType<typeof useChatApp>) => void }) {
  const api = useChatApp();
  useEffect(() => {
    props.onReady(api);
  }, [api, props]);
  return null;
}

function saveSessionsForBoot(sessions: Array<Record<string, unknown>>) {
  localStorage.setItem("openjax:web:sessions", JSON.stringify(sessions));
}

describe("useChatApp newChat guard", () => {
  beforeEach(() => {
    mocks.startSession
      .mockReset()
      .mockResolvedValueOnce({
        request_id: "req_new_1",
        session_id: "sess_new_1",
        timestamp: "2026-01-01T00:00:00Z"
      })
      .mockResolvedValueOnce({
        request_id: "req_new_2",
        session_id: "sess_new_2",
        timestamp: "2026-01-01T00:00:01Z"
      });
    mocks.submitTurn.mockReset().mockResolvedValue({
      request_id: "req_turn_1",
      session_id: "sess_new_1",
      turn_id: "turn_1",
      timestamp: "2026-01-01T00:00:02Z"
    });
    mocks.setPolicyLevel.mockReset().mockResolvedValue({ level: "allow" });
  });

  afterEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it("does not create another session when active session is already an empty draft", async () => {
    saveSessionsForBoot([
      {
        id: "sess_draft",
        title: "新聊天",
        createdAt: "2026-01-01T00:00:00Z",
        connection: "idle",
        turnPhase: "draft",
        lastEventSeq: 0,
        messages: [],
        pendingApprovals: []
      }
    ]);

    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });

    expect(mocks.startSession).not.toHaveBeenCalled();
    expect(apiRef!.state.sessions).toHaveLength(1);
    expect(apiRef!.state.infoToast).toBe("已在新对话中");
  });

  it("switches to local draft when active session already has messages", async () => {
    saveSessionsForBoot([
      {
        id: "sess_used",
        title: "你好",
        createdAt: "2026-01-01T00:00:00Z",
        connection: "idle",
        turnPhase: "completed",
        lastEventSeq: 2,
        messages: [
          {
            id: "msg_1",
            kind: "text",
            role: "user",
            content: "hello",
            timestamp: "2026-01-01T00:00:01Z"
          }
        ],
        pendingApprovals: []
      }
    ]);

    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });

    expect(mocks.startSession).not.toHaveBeenCalled();
    expect(apiRef!.state.sessions).toHaveLength(1);
    expect(apiRef!.state.activeSessionId).toBeNull();
  });

  it("keeps local draft when there is no active session", async () => {
    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });

    expect(mocks.startSession).not.toHaveBeenCalled();
    expect(apiRef!.state.sessions).toHaveLength(0);
    expect(apiRef!.state.activeSessionId).toBeNull();
  });

  it("creates remote session only when first message is sent from local draft", async () => {
    saveSessionsForBoot([
      {
        id: "sess_used",
        title: "你好",
        createdAt: "2026-01-01T00:00:00Z",
        connection: "idle",
        turnPhase: "completed",
        lastEventSeq: 2,
        messages: [
          {
            id: "msg_1",
            kind: "text",
            role: "user",
            content: "hello",
            timestamp: "2026-01-01T00:00:01Z"
          }
        ],
        pendingApprovals: []
      }
    ]);

    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });
    expect(mocks.startSession).not.toHaveBeenCalled();
    expect(apiRef!.state.activeSessionId).toBeNull();

    await act(async () => {
      await apiRef!.sendMessage("first message");
    });

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    expect(mocks.submitTurn).toHaveBeenCalledTimes(1);
    expect(apiRef!.state.activeSessionId).toBe("sess_new_1");
    expect(apiRef!.state.sessions[0]?.id).toBe("sess_new_1");
  });

  it("creates only one remote session when two sends race from local draft", async () => {
    const resolveStartSessionRef: {
      current: ((value: { request_id: string; session_id: string; timestamp: string }) => void) | null;
    } = { current: null };
    mocks.startSession.mockReset().mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStartSessionRef.current = resolve;
        })
    );
    mocks.submitTurn.mockReset().mockResolvedValue({
      request_id: "req_turn_race",
      session_id: "sess_race_1",
      turn_id: "turn_race_1",
      timestamp: "2026-01-01T00:00:03Z"
    });

    saveSessionsForBoot([
      {
        id: "sess_used",
        title: "你好",
        createdAt: "2026-01-01T00:00:00Z",
        connection: "idle",
        turnPhase: "completed",
        lastEventSeq: 2,
        messages: [
          {
            id: "msg_1",
            kind: "text",
            role: "user",
            content: "hello",
            timestamp: "2026-01-01T00:00:01Z"
          }
        ],
        pendingApprovals: []
      }
    ]);

    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });
    expect(apiRef!.state.activeSessionId).toBeNull();

    let send1: Promise<void> | null = null;
    let send2: Promise<void> | null = null;
    await act(async () => {
      send1 = apiRef!.sendMessage("first race");
      send2 = apiRef!.sendMessage("second race");
      await Promise.resolve();
    });

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    resolveStartSessionRef.current?.({
        request_id: "req_race_1",
        session_id: "sess_race_1",
        timestamp: "2026-01-01T00:00:00Z"
      });

    await act(async () => {
      await Promise.all([send1!, send2!]);
    });

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    expect(mocks.submitTurn).toHaveBeenCalledTimes(1);
    expect(apiRef!.state.infoToast).toBe(BUSY_TURN_BLOCKED_MESSAGE);
  });

  it("does not hijack active session when draft creation resolves after switching sessions", async () => {
    const resolveStartSessionRef: {
      current: ((value: { request_id: string; session_id: string; timestamp: string }) => void) | null;
    } = { current: null };
    mocks.startSession.mockReset().mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStartSessionRef.current = resolve;
        })
    );
    mocks.submitTurn.mockReset().mockResolvedValue({
      request_id: "req_turn_switch",
      session_id: "sess_new_switch",
      turn_id: "turn_switch",
      timestamp: "2026-01-01T00:00:03Z"
    });

    saveSessionsForBoot([
      {
        id: "sess_used",
        title: "老会话",
        createdAt: "2026-01-01T00:00:00Z",
        connection: "idle",
        turnPhase: "completed",
        lastEventSeq: 2,
        messages: [
          {
            id: "msg_1",
            kind: "text",
            role: "user",
            content: "hello",
            timestamp: "2026-01-01T00:00:01Z"
          }
        ],
        pendingApprovals: []
      }
    ]);

    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });
    expect(apiRef!.state.activeSessionId).toBeNull();

    const draftSend = apiRef!.sendMessage("draft message");
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.startSession).toHaveBeenCalledTimes(1);

    await act(async () => {
      apiRef!.switchSession("sess_used");
    });
    await act(async () => {
      await apiRef!.sendMessage("existing message");
    });

    resolveStartSessionRef.current?.({
        request_id: "req_switch",
        session_id: "sess_new_switch",
        timestamp: "2026-01-01T00:00:02Z"
      });
    await act(async () => {
      await draftSend;
    });

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    expect(mocks.submitTurn.mock.calls).toEqual([
      ["sess_used", "existing message"],
      ["sess_new_switch", "draft message"]
    ]);
    expect(apiRef!.state.activeSessionId).toBe("sess_used");
  });

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

  it("draft send with draftPolicyLevel=allow calls startSession then setPolicyLevel then submitTurn in order", async () => {
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

    await act(async () => { apiRef!.onPolicyLevelChange("allow"); });
    expect(apiRef!.draftPolicyLevel).toBe("allow");

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
});
