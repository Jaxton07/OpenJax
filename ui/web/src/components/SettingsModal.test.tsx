import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import SettingsModal from "./SettingsModal";

const providerItem = {
  provider_id: "provider_1",
  provider_name: "openai-main",
  base_url: "https://api.openai.com/v1",
  model_name: "gpt-4.1-mini",
  api_key_set: true,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z"
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("SettingsModal", () => {
  it("switches to provider tab and loads providers", async () => {
    const onListProviders = vi.fn().mockResolvedValue([providerItem]);
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
    expect(screen.getByText("已有 Provider")).toBeInTheDocument();
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
    await userEvent.type(screen.getByLabelText("名称"), "glm-main");
    await userEvent.type(
      screen.getByLabelText("Base URL"),
      "https://open.bigmodel.cn/api/anthropic"
    );
    await userEvent.type(screen.getByLabelText("模型名称"), "glm-4.7");
    await userEvent.type(screen.getByLabelText("API Key"), "key-a");
    await userEvent.click(screen.getByRole("button", { name: "新增 Provider" }));

    await waitFor(() =>
      expect(onCreateProvider).toHaveBeenCalledWith({
        providerName: "glm-main",
        baseUrl: "https://open.bigmodel.cn/api/anthropic",
        modelName: "glm-4.7",
        apiKey: "key-a"
      })
    );
    expect(await screen.findByText("Provider 创建成功。")).toBeInTheDocument();
  });

  it("selects card then updates provider", async () => {
    const onUpdateProvider = vi.fn().mockResolvedValue({
      ...providerItem,
      model_name: "gpt-4.1"
    });

    render(
      <SettingsModal
        open
        initialSettings={{ baseUrl: "http://127.0.0.1:8765", outputMode: "sse" }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={async () => [providerItem]}
        onCreateProvider={async () => {
          throw new Error("not used");
        }}
        onUpdateProvider={onUpdateProvider}
        onDeleteProvider={async () => {}}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await screen.findByText("openai-main");
    await userEvent.click(screen.getByRole("button", { name: /openai-main/i }));

    const modelInput = screen.getByLabelText("模型名称");
    await userEvent.clear(modelInput);
    await userEvent.type(modelInput, "gpt-4.1");
    await userEvent.click(screen.getByRole("button", { name: "保存修改" }));

    await waitFor(() =>
      expect(onUpdateProvider).toHaveBeenCalledWith("provider_1", {
        providerName: "openai-main",
        baseUrl: "https://api.openai.com/v1",
        modelName: "gpt-4.1",
        apiKey: ""
      })
    );
  });

  it("deletes provider after confirmation", async () => {
    const onDeleteProvider = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(
      <SettingsModal
        open
        initialSettings={{ baseUrl: "http://127.0.0.1:8765", outputMode: "sse" }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={async () => [providerItem]}
        onCreateProvider={async () => {
          throw new Error("not used");
        }}
        onUpdateProvider={async () => {
          throw new Error("not used");
        }}
        onDeleteProvider={onDeleteProvider}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await screen.findByText("openai-main");
    await userEvent.click(screen.getByRole("button", { name: "删除" }));

    await waitFor(() => expect(onDeleteProvider).toHaveBeenCalledWith("provider_1"));
    expect(await screen.findByText("Provider 已删除。")).toBeInTheDocument();
  });
});
