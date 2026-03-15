import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import SettingsModal from "./SettingsModal";

describe("SettingsModal", () => {
  it("switches to provider tab and loads providers", async () => {
    const onListProviders = vi.fn().mockResolvedValue([
      {
        provider_id: "provider_1",
        provider_name: "openai-main",
        base_url: "https://api.openai.com/v1",
        model_name: "gpt-4.1-mini",
        api_key_set: true,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z"
      }
    ]);
    render(
      <SettingsModal
        open
        initialSettings={{ baseUrl: "http://127.0.0.1:8765", outputMode: "sse" }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={onListProviders}
        onCreateProvider={async () => {
          throw new Error("not used");
        }}
        onUpdateProvider={async () => {
          throw new Error("not used");
        }}
        onDeleteProvider={async () => {}}
      />
    );
    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await waitFor(() => expect(onListProviders).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("openai-main")).toBeInTheDocument();
  });

  it("submits create provider form", async () => {
    const onCreateProvider = vi.fn().mockResolvedValue({
      provider_id: "provider_2",
      provider_name: "glm-main",
      base_url: "https://open.bigmodel.cn/api/anthropic",
      model_name: "glm-4.7",
      api_key_set: true,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z"
    });
    render(
      <SettingsModal
        open
        initialSettings={{ baseUrl: "http://127.0.0.1:8765", outputMode: "sse" }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={async () => []}
        onCreateProvider={onCreateProvider}
        onUpdateProvider={async () => {
          throw new Error("not used");
        }}
        onDeleteProvider={async () => {}}
      />
    );
    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    const inputs = screen.getAllByRole("textbox");
    await userEvent.type(inputs[0], "glm-main");
    await userEvent.type(inputs[1], "https://open.bigmodel.cn/api/anthropic");
    await userEvent.type(inputs[2], "glm-4.7");
    await userEvent.type(inputs[3], "key-a");
    await userEvent.click(screen.getByRole("button", { name: "新增 Provider" }));
    await waitFor(() =>
      expect(onCreateProvider).toHaveBeenCalledWith({
        providerName: "glm-main",
        baseUrl: "https://open.bigmodel.cn/api/anthropic",
        modelName: "glm-4.7",
        apiKey: "key-a"
      })
    );
  });
});
