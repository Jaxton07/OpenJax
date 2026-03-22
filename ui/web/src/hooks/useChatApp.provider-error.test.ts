import { act, render, screen, waitFor } from "@testing-library/react";
import { createElement, useEffect } from "react";
import { describe, expect, it, vi } from "vitest";
import { useChatApp } from "./useChatApp";

const mocks = vi.hoisted(() => {
  const startSession = vi.fn().mockResolvedValue({
    request_id: "req_1",
    session_id: "sess_1",
    timestamp: "2026-01-01T00:00:00Z"
  });
  const submitTurn = vi.fn().mockRejectedValue(
    Object.assign(new Error("upstream error: provider returned HTTP 404"), {
      status: 404,
      code: "UPSTREAM_UNAVAILABLE",
      retryable: false
    })
  );
  return { startSession, submitTurn };
});

vi.mock("../lib/gatewayClient", () => ({
  GatewayClient: vi.fn().mockImplementation(() => ({
    startSession: mocks.startSession,
    submitTurn: mocks.submitTurn
  }))
}));

function HookHarness(props: { onReady: (api: ReturnType<typeof useChatApp>) => void }) {
  const api = useChatApp();

  useEffect(() => {
    props.onReady(api);
  }, [api, props]);

  if (!api.state.globalError) {
    return null;
  }
  return createElement("div", { role: "alert" }, api.state.globalError);
}

describe("useChatApp provider errors", () => {
  it("shows upstream provider 404 details when submitTurn fails", async () => {
    let apiRef: ReturnType<typeof useChatApp> | null = null;

    render(createElement(HookHarness, { onReady: (api) => (apiRef = api) }));

    await waitFor(() => expect(apiRef).not.toBeNull());

    await act(async () => {
      await apiRef!.sendMessage("hello");
    });

    await waitFor(() => {
      const text = screen.getByRole("alert").textContent ?? "";
      expect(text).toContain("404");
      expect(text.toLowerCase()).toContain("upstream");
    });
  });
});
