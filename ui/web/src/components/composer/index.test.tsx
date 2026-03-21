import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
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
        onCompact={vi.fn()}
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

    await userEvent.type(screen.getByPlaceholderText("有问题，尽管问"), "/");

    expect(await screen.findByText("/help")).toBeInTheDocument();
  });
});
