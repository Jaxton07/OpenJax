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
  updated_at: "2026-01-01T00:00:00Z",
  provider_type: "custom" as const,
  context_window_size: 128000
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
        initialSettings={{
          baseUrl: "http://127.0.0.1:8765",
          outputMode: "sse",
          selectedProviderId: null,
          selectedModelName: null
        }}
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
        onGetActiveProvider={async () => providerItem}
        onSetActiveProvider={async () => providerItem}
        onFetchCatalog={async () => []}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));

    await waitFor(() => expect(onListProviders).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("openai-main")).toBeInTheDocument();
    expect(screen.getByText("Provider List")).toBeInTheDocument();
  });

  it("submits create provider form", async () => {
    const onCreateProvider = vi.fn().mockResolvedValue({
      provider_id: "provider_2",
      provider_name: "glm-main",
      base_url: "https://open.bigmodel.cn/api/anthropic",
      model_name: "glm-4.7",
      api_key_set: true,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
      provider_type: "custom" as const,
      context_window_size: 0
    });

    render(
      <SettingsModal
        open
        initialSettings={{
          baseUrl: "http://127.0.0.1:8765",
          outputMode: "sse",
          selectedProviderId: null,
          selectedModelName: null
        }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={async () => []}
        onCreateProvider={onCreateProvider}
        onUpdateProvider={async () => {
          throw new Error("not used");
        }}
        onDeleteProvider={async () => {}}
        onGetActiveProvider={async () => null}
        onSetActiveProvider={async () => providerItem}
        onFetchCatalog={async () => []}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await userEvent.click(screen.getByRole("button", { name: "Add Provider" }));
    await userEvent.type(screen.getByLabelText("名称"), "glm-main");
    await userEvent.type(
      screen.getByLabelText("Base URL"),
      "https://open.bigmodel.cn/api/anthropic"
    );
    await userEvent.type(screen.getByLabelText("模型名称"), "glm-4.7");
    await userEvent.type(screen.getByLabelText("上下文窗口大小"), "128000");
    await userEvent.type(screen.getByLabelText("API Key"), "key-a");
    await userEvent.click(screen.getByRole("button", { name: "新增 Provider" }));

    await waitFor(() =>
      expect(onCreateProvider).toHaveBeenCalledWith({
        providerName: "glm-main",
        baseUrl: "https://open.bigmodel.cn/api/anthropic",
        modelName: "glm-4.7",
        apiKey: "key-a",
        providerType: "custom",
        contextWindowSize: 128000
      })
    );
  });

  it("selects provider card to activate it", async () => {
    const onSave = vi.fn();
    const onSetActiveProvider = vi.fn().mockResolvedValue(providerItem);
    render(
      <SettingsModal
        open
        initialSettings={{
          baseUrl: "http://127.0.0.1:8765",
          outputMode: "sse",
          selectedProviderId: null,
          selectedModelName: null
        }}
        onClose={() => {}}
        onSave={onSave}
        onTest={async () => true}
        onListProviders={async () => [providerItem]}
        onCreateProvider={async () => {
          throw new Error("not used");
        }}
        onUpdateProvider={async () => {
          throw new Error("not used");
        }}
        onDeleteProvider={async () => {}}
        onGetActiveProvider={async () => null}
        onSetActiveProvider={onSetActiveProvider}
        onFetchCatalog={async () => []}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await screen.findByText("openai-main");
    await userEvent.click(screen.getByRole("button", { name: /openai-main/i }));

    await waitFor(() => expect(onSetActiveProvider).toHaveBeenCalledWith("provider_1"));
    expect(onSave).toHaveBeenCalledWith({
      baseUrl: "http://127.0.0.1:8765",
      outputMode: "sse",
      selectedProviderId: "provider_1",
      selectedModelName: "gpt-4.1-mini"
    });
  });

  it("opens edit panel only when clicking edit", async () => {
    const onUpdateProvider = vi.fn().mockResolvedValue({
      ...providerItem,
      model_name: "gpt-4.1"
    });

    render(
      <SettingsModal
        open
        initialSettings={{
          baseUrl: "http://127.0.0.1:8765",
          outputMode: "sse",
          selectedProviderId: "provider_1",
          selectedModelName: "gpt-4.1-mini"
        }}
        onClose={() => {}}
        onSave={() => {}}
        onTest={async () => true}
        onListProviders={async () => [providerItem]}
        onCreateProvider={async () => {
          throw new Error("not used");
        }}
        onUpdateProvider={onUpdateProvider}
        onDeleteProvider={async () => {}}
        onGetActiveProvider={async () => providerItem}
        onSetActiveProvider={async () => providerItem}
        onFetchCatalog={async () => []}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await screen.findByText("openai-main");
    expect(screen.queryByText("编辑 Provider")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "编辑" }));

    const modelInput = screen.getByLabelText("模型名称");
    await userEvent.clear(modelInput);
    await userEvent.type(modelInput, "gpt-4.1");
    await userEvent.click(screen.getByRole("button", { name: "保存修改" }));

    await waitFor(() =>
      expect(onUpdateProvider).toHaveBeenCalledWith("provider_1", {
        providerName: "openai-main",
        baseUrl: "https://api.openai.com/v1",
        modelName: "gpt-4.1",
        apiKey: "",
        providerType: "custom",
        contextWindowSize: 128000
      })
    );
  });

  it("deletes provider after confirmation", async () => {
    const onDeleteProvider = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(
      <SettingsModal
        open
        initialSettings={{
          baseUrl: "http://127.0.0.1:8765",
          outputMode: "sse",
          selectedProviderId: "provider_1",
          selectedModelName: "gpt-4.1-mini"
        }}
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
        onGetActiveProvider={async () => providerItem}
        onSetActiveProvider={async () => providerItem}
        onFetchCatalog={async () => []}
      />
    );

    await userEvent.click(screen.getByRole("button", { name: "LLM Provider" }));
    await screen.findByText("openai-main");
    await userEvent.click(screen.getByRole("button", { name: "删除" }));

    await waitFor(() => expect(onDeleteProvider).toHaveBeenCalledWith("provider_1"));
  });
});
