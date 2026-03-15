import { useEffect, useMemo, useRef, useState } from "react";
import type { AppSettings, LlmProvider } from "../types/gateway";
import GeneralSettingsPanel, { type GeneralStatus } from "./settings/GeneralSettingsPanel";
import ProviderForm, { type ProviderFormValue } from "./settings/ProviderForm";
import ProviderListPanel from "./settings/ProviderListPanel";
import SettingsSidebar from "./settings/SettingsSidebar";

const PROVIDER_FORM_EXIT_MS = 580;
const PROVIDER_CLOSE_ICON = new URL("../pic/icon/close.svg", import.meta.url).href;

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

  const [providerError, setProviderError] = useState<string>("");
  const [providerSuccess, setProviderSuccess] = useState<string>("");
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
      setProviderError("");
      setProviderSuccess("");
      setSelectedProviderId(props.initialSettings.selectedProviderId);
      setActiveProviderId(props.initialSettings.selectedProviderId);
      setProviderPanelMode("none");
      setClosingProviderPanel(false);
    }
    wasOpenRef.current = props.open;
  }, [props.initialSettings, props.open]);

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
    setProviderError("");
    try {
      const [items, activeProvider] = await Promise.all([
        props.onListProviders(),
        props.onGetActiveProvider()
      ]);
      const activeId = activeProvider?.provider_id ?? null;
      setProviders(items);
      setActiveProviderId(activeId);
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
      setProviderError((error as Error).message);
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
    if (!providerSuccess) {
      return;
    }
    const timer = window.setTimeout(() => setProviderSuccess(""), 2200);
    return () => window.clearTimeout(timer);
  }, [providerSuccess]);

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
    providerPanelMode === "create"
      ? {
        providerName: "",
        baseUrl: "",
        modelName: "",
        apiKey: ""
      }
      : selectedProvider
        ? {
            providerName: selectedProvider.provider_name,
            baseUrl: selectedProvider.base_url,
            modelName: selectedProvider.model_name,
            apiKey: ""
          }
        : {
            providerName: "",
            baseUrl: "",
            modelName: "",
            apiKey: ""
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
    setProviderError("");
    setProviderSuccess("");
    try {
      const created = await props.onCreateProvider(value);
      setProviders((prev) => [created, ...prev]);
      setSelectedProviderId(created.provider_id);
      setProviderPanelMode("edit");
    } catch (error) {
      setProviderError((error as Error).message);
    } finally {
      setSavingProvider(false);
    }
  };

  const updateProvider = async (value: ProviderFormValue) => {
    if (!selectedProvider) {
      return;
    }
    setSavingProvider(true);
    setProviderError("");
    setProviderSuccess("");
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
      setProviderError((error as Error).message);
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
    setProviderError("");
    setProviderSuccess("");
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
      setProviderError((error as Error).message);
    } finally {
      setSavingProvider(false);
    }
  };

  const activateProvider = async (provider: LlmProvider) => {
    setProviderError("");
    setProviderSuccess("");
    if (activeProviderId === provider.provider_id) {
      setProviderSuccess("当前 Provider 已在使用中，新会话将继续使用该配置。");
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
      setProviderSuccess("已切换 Provider，将在新会话中生效。");
    } catch (error) {
      setProviderError((error as Error).message);
    }
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
  };

  const providerNoticeMessage = providerError || providerSuccess;
  const providerNoticeTone: "error" | "success" | "info" = providerError
    ? "error"
    : providerSuccess
      ? "success"
      : "info";

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
                  noticeMessage={providerNoticeMessage}
                  noticeTone={providerNoticeTone}
                  onRefresh={refreshProviders}
                  onAddProvider={() => {
                    openProviderPanel("create");
                    setProviderError("");
                    setProviderSuccess("");
                  }}
                  onSelect={(provider) => void activateProvider(provider)}
                  onEdit={(provider) => {
                    setSelectedProviderId(provider.provider_id);
                    openProviderPanel("edit");
                    setProviderError("");
                    setProviderSuccess("");
                  }}
                  onDelete={removeProvider}
                />
                {providerPanelMode !== "none" ? (
                  <div
                    className={
                      closingProviderPanel
                        ? "provider-form-wrap provider-form-wrap-closing"
                        : "provider-form-wrap"
                    }
                    data-opened="true"
                  >
                    <button
                      type="button"
                      className="provider-form-close-btn"
                      aria-label="关闭新增/编辑面板"
                      onClick={closeProviderPanel}
                    >
                      <img src={PROVIDER_CLOSE_ICON} alt="" />
                    </button>
                    <ProviderForm
                      mode={providerPanelMode === "edit" ? "edit" : "create"}
                      initialValue={providerFormInitialValue}
                      submitting={savingProvider}
                      onSubmit={providerPanelMode === "edit" ? updateProvider : createProvider}
                      onCancelEdit={closeProviderPanel}
                    />
                  </div>
                ) : null}
              </section>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
