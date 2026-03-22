import { useEffect, useMemo, useRef, useState } from "react";
import type { AppSettings, CatalogProvider, LlmProvider } from "../types/gateway";
import GeneralSettingsPanel, { type GeneralStatus } from "./settings/GeneralSettingsPanel";
import ProviderEditorPanel from "./settings/ProviderEditorPanel";
import type { ProviderFormValue } from "./settings/ProviderForm";
import ProviderListPanel from "./settings/ProviderListPanel";
import SettingsSidebar from "./settings/SettingsSidebar";

const PROVIDER_FORM_EXIT_MS = 580;
const PROVIDER_TOAST_DURATION_MS = 2200;

type ProviderToastTone = "success" | "error" | "info";

interface ProviderToastState {
  id: number;
  message: string;
  tone: ProviderToastTone;
  durationMs: number;
}

interface SettingsModalProps {
  open: boolean;
  initialSettings: AppSettings;
  onClose: () => void;
  onSave: (settings: AppSettings) => void;
  onTest: (settings: AppSettings) => Promise<boolean>;
  onListProviders: () => Promise<LlmProvider[]>;
  onCreateProvider: (payload: ProviderFormValue) => Promise<LlmProvider>;
  onUpdateProvider: (providerId: string, payload: ProviderFormValue) => Promise<LlmProvider>;
  onDeleteProvider: (providerId: string) => Promise<void>;
  onGetActiveProvider: () => Promise<LlmProvider | null>;
  onSetActiveProvider: (providerId: string) => Promise<LlmProvider>;
  onFetchCatalog: () => Promise<CatalogProvider[]>;
}

const IDLE_GENERAL_STATUS: GeneralStatus = {
  tone: "idle",
  message: "你可以先测试连接，再保存设置。"
};

export default function SettingsModal(props: SettingsModalProps) {
  const [draft, setDraft] = useState<AppSettings>(props.initialSettings);
  const [activeTab, setActiveTab] = useState<"general" | "provider">("general");

  const [generalStatus, setGeneralStatus] = useState<GeneralStatus>(IDLE_GENERAL_STATUS);
  const [testingGeneral, setTestingGeneral] = useState(false);
  const [savingGeneral, setSavingGeneral] = useState(false);

  const [providerToast, setProviderToast] = useState<ProviderToastState | null>(null);
  const [loadingProviders, setLoadingProviders] = useState(false);
  const [savingProvider, setSavingProvider] = useState(false);
  const [providers, setProviders] = useState<LlmProvider[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(
    props.initialSettings.selectedProviderId
  );
  const [activeProviderId, setActiveProviderId] = useState<string | null>(
    props.initialSettings.selectedProviderId
  );
  const [providerPanelMode, setProviderPanelMode] = useState<"none" | "create" | "edit">("none");
  const [closingProviderPanel, setClosingProviderPanel] = useState(false);
  const [catalog, setCatalog] = useState<CatalogProvider[]>([]);
  const [pendingCatalogEntry, setPendingCatalogEntry] = useState<CatalogProvider | null>(null);
  const wasOpenRef = useRef(false);

  useEffect(() => {
    if (!props.open) {
      return;
    }
    const onEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        props.onClose();
      }
    };
    window.addEventListener("keydown", onEscape);
    return () => window.removeEventListener("keydown", onEscape);
  }, [props.open, props.onClose]);

  useEffect(() => {
    if (props.open && !wasOpenRef.current) {
      setDraft(props.initialSettings);
      setActiveTab("general");
      setGeneralStatus(IDLE_GENERAL_STATUS);
      setTestingGeneral(false);
      setSavingGeneral(false);
      setProviderToast(null);
      setSelectedProviderId(props.initialSettings.selectedProviderId);
      setActiveProviderId(props.initialSettings.selectedProviderId);
      setProviderPanelMode("none");
      setClosingProviderPanel(false);
    }
    wasOpenRef.current = props.open;
  }, [props.initialSettings, props.open]);

  const showProviderToast = (
    message: string,
    tone: ProviderToastTone = "info",
    durationMs: number = PROVIDER_TOAST_DURATION_MS
  ) => {
    setProviderToast({
      id: Date.now(),
      message,
      tone,
      durationMs
    });
  };

  useEffect(() => {
    if (!providerToast) {
      return;
    }
    const { id, durationMs } = providerToast;
    const timer = window.setTimeout(() => {
      setProviderToast((current) => (current?.id === id ? null : current));
    }, durationMs);
    return () => window.clearTimeout(timer);
  }, [providerToast]);

  const syncSelectedProviderSettings = (provider: LlmProvider | null, persist: boolean) => {
    const nextDraft: AppSettings = {
      ...draft,
      selectedProviderId: provider?.provider_id ?? null,
      selectedModelName: provider?.model_name ?? null
    };
    setDraft(nextDraft);
    if (persist) {
      const nextPersisted: AppSettings = {
        ...props.initialSettings,
        selectedProviderId: provider?.provider_id ?? null,
        selectedModelName: provider?.model_name ?? null
      };
      props.onSave(nextPersisted);
    }
  };

  const refreshProviders = async () => {
    setLoadingProviders(true);
    setProviderToast(null);
    try {
      const [items, activeProvider, catalogData] = await Promise.all([
        props.onListProviders(),
        props.onGetActiveProvider(),
        props.onFetchCatalog()
      ]);
      const activeId = activeProvider?.provider_id ?? null;
      setProviders(items);
      setActiveProviderId(activeId);
      setCatalog(catalogData);
      setSelectedProviderId((current) => {
        if (current && items.some((item) => item.provider_id === current)) {
          return current;
        }
        return activeId;
      });
      if (activeProvider) {
        syncSelectedProviderSettings(activeProvider, false);
      }
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    } finally {
      setLoadingProviders(false);
    }
  };

  useEffect(() => {
    if (!props.open || activeTab !== "provider") {
      return;
    }
    void refreshProviders();
  }, [activeTab, props.open]);

  useEffect(() => {
    if (!closingProviderPanel) {
      return;
    }
    const timer = window.setTimeout(() => {
      setProviderPanelMode("none");
      setClosingProviderPanel(false);
    }, PROVIDER_FORM_EXIT_MS);
    return () => window.clearTimeout(timer);
  }, [closingProviderPanel]);

  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.provider_id === selectedProviderId) ?? null,
    [providers, selectedProviderId]
  );

  const providerFormInitialValue: ProviderFormValue =
    providerPanelMode === "create" && pendingCatalogEntry
      ? {
          providerName: pendingCatalogEntry.display_name,
          baseUrl: pendingCatalogEntry.base_url,
          modelName: pendingCatalogEntry.default_model,
          apiKey: "",
          providerType: "built_in",
          contextWindowSize:
            pendingCatalogEntry.models.find(
              (m) => m.model_id === pendingCatalogEntry.default_model
            )?.context_window ?? 0,
          catalogModels: pendingCatalogEntry.models
        }
      : providerPanelMode === "create"
      ? {
          providerName: "",
          baseUrl: "",
          modelName: "",
          apiKey: "",
          providerType: "custom",
          contextWindowSize: 0
        }
      : selectedProvider
        ? {
            providerName: selectedProvider.provider_name,
            baseUrl: selectedProvider.base_url,
            modelName: selectedProvider.model_name,
            apiKey: "",
            providerType: selectedProvider.provider_type,
            contextWindowSize: selectedProvider.context_window_size,
            catalogModels:
              selectedProvider.provider_type === "built_in"
                ? catalog.find(
                    (e) =>
                      e.base_url.replace(/\/+$/, "") ===
                      selectedProvider.base_url.replace(/\/+$/, "")
                  )?.models
                : undefined
          }
        : {
            providerName: "",
            baseUrl: "",
            modelName: "",
            apiKey: "",
            providerType: "custom",
            contextWindowSize: 0
          };

  const handleSaveSettings = async (settings: AppSettings) => {
    setSavingGeneral(true);
    setGeneralStatus({ tone: "info", message: "正在保存设置..." });
    try {
      props.onSave(settings);
      setGeneralStatus({ tone: "success", message: "设置已保存。" });
    } catch (error) {
      setGeneralStatus({ tone: "error", message: (error as Error).message });
    } finally {
      setSavingGeneral(false);
    }
  };

  const handleTestSettings = async (settings: AppSettings) => {
    setTestingGeneral(true);
    setGeneralStatus({ tone: "info", message: "正在测试连接..." });
    try {
      const ok = await props.onTest(settings);
      setGeneralStatus({
        tone: ok ? "success" : "error",
        message: ok ? "连接测试成功。" : "连接测试失败，请检查配置。"
      });
    } catch (error) {
      setGeneralStatus({ tone: "error", message: (error as Error).message });
    } finally {
      setTestingGeneral(false);
    }
  };

  const createProvider = async (value: ProviderFormValue) => {
    setSavingProvider(true);
    setProviderToast(null);
    try {
      const created = await props.onCreateProvider(value);
      setProviders((prev) => [created, ...prev]);
      setSelectedProviderId(created.provider_id);
      setProviderPanelMode("edit");
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    } finally {
      setSavingProvider(false);
    }
  };

  const updateProvider = async (value: ProviderFormValue) => {
    if (!selectedProvider) {
      return;
    }
    setSavingProvider(true);
    setProviderToast(null);
    try {
      const updated = await props.onUpdateProvider(selectedProvider.provider_id, value);
      setProviders((prev) =>
        prev.map((item) => (item.provider_id === updated.provider_id ? updated : item))
      );
      if (activeProviderId === updated.provider_id) {
        await props.onSetActiveProvider(updated.provider_id);
        syncSelectedProviderSettings(updated, true);
      }
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    } finally {
      setSavingProvider(false);
    }
  };

  const removeProvider = async (provider: LlmProvider) => {
    const confirmed = window.confirm(`确认删除 Provider「${provider.provider_name}」？`);
    if (!confirmed) {
      return;
    }
    setSavingProvider(true);
    setProviderToast(null);
    try {
      await props.onDeleteProvider(provider.provider_id);
      setProviders((prev) => prev.filter((item) => item.provider_id !== provider.provider_id));
      setSelectedProviderId((current) => (current === provider.provider_id ? null : current));
      if (activeProviderId === provider.provider_id) {
        setActiveProviderId(null);
        syncSelectedProviderSettings(null, true);
      }
      if (providerPanelMode === "edit" && selectedProviderId === provider.provider_id) {
        setClosingProviderPanel(true);
      }
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    } finally {
      setSavingProvider(false);
    }
  };

  const activateProvider = async (provider: LlmProvider) => {
    setProviderToast(null);
    if (activeProviderId === provider.provider_id) {
      showProviderToast("当前 Provider 已在使用中，新会话将继续使用该配置。", "info");
      return;
    }
    setSelectedProviderId(provider.provider_id);
    try {
      const active = await props.onSetActiveProvider(provider.provider_id);
      setActiveProviderId(active.provider_id);
      setProviders((prev) =>
        prev.map((item) => (item.provider_id === active.provider_id ? active : item))
      );
      syncSelectedProviderSettings(active, true);
      showProviderToast("已切换 Provider，将在新会话中生效。", "success");
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    }
  };

  const handleSwitchModel = async (
    providerId: string,
    modelId: string,
    contextWindow: number
  ) => {
    const provider = providers.find((p) => p.provider_id === providerId);
    if (!provider) return;
    try {
      const updated = await props.onUpdateProvider(providerId, {
        providerName: provider.provider_name,
        baseUrl: provider.base_url,
        modelName: modelId,
        apiKey: "",
        providerType: provider.provider_type,
        contextWindowSize: contextWindow
      });
      setProviders((prev) =>
        prev.map((p) => (p.provider_id === updated.provider_id ? updated : p))
      );
      if (activeProviderId === updated.provider_id) {
        syncSelectedProviderSettings(updated, true);
      }
      showProviderToast(`已切换模型为 ${modelId}，将在新会话中生效。`, "success");
    } catch (error) {
      showProviderToast((error as Error).message, "error");
    }
  };

  const handleConfigureCatalogEntry = (entry: CatalogProvider) => {
    openProviderPanel("create");
    setProviderToast(null);
    setPendingCatalogEntry(entry);
  };

  useEffect(() => {
    if (providerPanelMode === "edit" && !selectedProvider) {
      setClosingProviderPanel(true);
    }
  }, [providerPanelMode, selectedProvider]);

  const openProviderPanel = (mode: "create" | "edit") => {
    setProviderPanelMode(mode);
    setClosingProviderPanel(false);
  };

  const closeProviderPanel = () => {
    if (providerPanelMode === "none" || closingProviderPanel) {
      return;
    }
    setClosingProviderPanel(true);
    setPendingCatalogEntry(null);
  };

  if (!props.open) {
    return null;
  }

  return (
    <div className="modal-backdrop" onClick={props.onClose}>
      <div className="settings-modal" onClick={(event) => event.stopPropagation()}>
        <header className="modal-header">
          <div className="modal-title-group">
            <h2>设置</h2>
            <p>管理连接参数、输出策略与模型 Provider。</p>
          </div>
          <button type="button" className="btn-secondary" onClick={props.onClose}>
            关闭
          </button>
        </header>
        {providerToast && activeTab === "provider" ? (
          <div
            className={`provider-floating-toast provider-floating-toast-${providerToast.tone}`}
            role="status"
            aria-live="polite"
          >
            <span className="provider-floating-toast__message">{providerToast.message}</span>
            <div className="provider-floating-toast__progress-track">
              <div
                key={providerToast.id}
                className="provider-floating-toast__progress-bar"
                style={{ animationDuration: `${providerToast.durationMs}ms` }}
              />
            </div>
          </div>
        ) : null}

        <div className="settings-modal-body">
          <SettingsSidebar activeTab={activeTab} onChangeTab={setActiveTab} />

          <div className="settings-modal-content">
            {activeTab === "general" ? (
              <GeneralSettingsPanel
                draft={draft}
                status={generalStatus}
                testing={testingGeneral}
                saving={savingGeneral}
                onChangeDraft={setDraft}
                onTest={handleTestSettings}
                onSave={handleSaveSettings}
              />
            ) : (
              <section
                className={`settings-panel provider-panel ${
                  providerPanelMode === "none" || closingProviderPanel ? "is-list-only" : "is-with-form"
                }`}
              >
                <ProviderListPanel
                  providers={providers}
                  loading={loadingProviders}
                  selectedProviderId={activeProviderId}
                  catalog={catalog}
                  onRefresh={refreshProviders}
                  onAddProvider={() => {
                    openProviderPanel("create");
                    setProviderToast(null);
                    setPendingCatalogEntry(null);
                  }}
                  onSelect={(provider) => void activateProvider(provider)}
                  onEdit={(provider) => {
                    setSelectedProviderId(provider.provider_id);
                    openProviderPanel("edit");
                    setProviderToast(null);
                  }}
                  onDelete={removeProvider}
                  onSwitchModel={handleSwitchModel}
                  onConfigureCatalogEntry={handleConfigureCatalogEntry}
                />
                {providerPanelMode !== "none" ? (
                  <ProviderEditorPanel
                    closing={closingProviderPanel}
                    mode={providerPanelMode === "edit" ? "edit" : "create"}
                    initialValue={providerFormInitialValue}
                    submitting={savingProvider}
                    scrollResetKey={`${providerPanelMode}:${selectedProviderId ?? "create"}`}
                    onClose={closeProviderPanel}
                    onSubmit={providerPanelMode === "edit" ? updateProvider : createProvider}
                  />
                ) : null}
              </section>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
