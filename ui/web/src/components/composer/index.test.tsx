import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { applySessionEvent } from "../../lib/session-events/reducer";
import type { ChatSession } from "../../types/chat";
import Composer from "./index";

describe("Composer slash commands", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
    delete (window as Window & { __GATEWAY_URL__?: string }).__GATEWAY_URL__;
  });

  it("shows slash command suggestions before a session exists", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        commands: [
          {
            name: "help",
            aliases: ["?"],
            description: "Show help",
            usage_hint: "/help",
            kind: "builtin",
            replaces_input: false,
          },
        ],
      }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <Composer
        baseUrl="http://127.0.0.1:8765"
        accessToken="token-123"
        sessionId={null}
        onSend={vi.fn()}
        onNewChat={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "http://127.0.0.1:8765/api/v1/slash_commands",
        expect.objectContaining({
          headers: { Authorization: "Bearer token-123" },
        })
      );
    });

    await userEvent.type(
      screen.getByPlaceholderText("Ask anything (type / for commands)"),
      "/"
    );

    expect(await screen.findByText("/help")).toBeInTheDocument();
  });

  it("shows context usage ring for the active session", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ commands: [] }),
      })
    );

    render(
      <Composer
        baseUrl="http://127.0.0.1:8765"
        accessToken="token-123"
        sessionId="sess_ctx"
        contextUsage={{
          ratio: 0.625,
          inputTokens: 5000,
          contextWindowSize: 8000,
          updatedAt: "2026-03-21T00:00:01Z",
        }}
        onSend={vi.fn()}
        onNewChat={vi.fn()}
      />
    );

    expect(screen.getByText("上下文使用 62.5% · 5,000 / 8,000 tokens")).toBeInTheDocument();
  });

  it("stores context usage updates in session state", () => {
    const session: ChatSession = {
      id: "sess_ctx",
      title: "Context session",
      createdAt: "2026-03-21T00:00:00Z",
      connection: "active",
      turnPhase: "draft",
      lastEventSeq: 0,
      messages: [],
      pendingApprovals: [],
    };

    const next = applySessionEvent(session, {
      request_id: "req_ctx",
      session_id: "sess_ctx",
      event_seq: 1,
      timestamp: "2026-03-21T00:00:01Z",
      type: "context_usage_updated",
      payload: {
        ratio: 0.5,
        input_tokens: 2048,
        context_window_size: 4096,
        updated_at: "2026-03-21T00:00:01Z",
      },
    } as any);

    expect(next.contextUsage).toEqual({
      ratio: 0.5,
      inputTokens: 2048,
      contextWindowSize: 4096,
      updatedAt: "2026-03-21T00:00:01Z",
    });
  });
});
