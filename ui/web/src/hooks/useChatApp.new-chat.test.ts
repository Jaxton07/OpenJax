import { act, render, waitFor } from "@testing-library/react";
import { createElement, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChatApp } from "./useChatApp";

const mocks = vi.hoisted(() => {
  const startSession = vi.fn();
  return { startSession };
});

vi.mock("../lib/gatewayClient", () => ({
  GatewayClient: vi.fn().mockImplementation(() => ({
    startSession: mocks.startSession
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

  it("creates new session when active session already has messages", async () => {
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

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    expect(apiRef!.state.sessions).toHaveLength(2);
    expect(apiRef!.state.activeSessionId).toBe("sess_new_1");
  });

  it("creates new session when there is no active session", async () => {
    let apiRef: ReturnType<typeof useChatApp> | null = null;
    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));
    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.newChat();
    });

    expect(mocks.startSession).toHaveBeenCalledTimes(1);
    expect(apiRef!.state.sessions[0]?.id).toBe("sess_new_1");
    expect(apiRef!.state.activeSessionId).toBe("sess_new_1");
  });
});
