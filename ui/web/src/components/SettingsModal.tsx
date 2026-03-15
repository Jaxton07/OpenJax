import { useEffect, useMemo, useState } from "react";
import type { AppSettings, LlmProvider } from "../types/gateway";
import GeneralSettingsPanel, { type GeneralStatus } from "./settings/GeneralSettingsPanel";
import ProviderForm, { type ProviderFormValue } from "./settings/ProviderForm";
import ProviderListPanel from "./settings/ProviderListPanel";
import SettingsSidebar from "./settings/SettingsSidebar";

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

  const [providerStatus, setProviderStatus] = useState<string>("");
  const [loadingProviders, setLoadingProviders] = useState(false);
  const [savingProvider, setSavingProvider] = useState(false);
  const [providers, setProviders] = useState<LlmProvider[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);

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
    if (props.open) {
      setDraft(props.initialSettings);
      setActiveTab("general");
      setGeneralStatus(IDLE_GENERAL_STATUS);
      setTestingGeneral(false);
      setSavingGeneral(false);
      setProviderStatus("");
      setSelectedProviderId(null);
    }
  }, [props.initialSettings, props.open]);

  const refreshProviders = async () => {
    setLoadingProviders(true);
    setProviderStatus("正在加载 Provider 列表...");
    try {
      const items = await props.onListProviders();
      setProviders(items);
      setSelectedProviderId((current) =>
        current && items.some((item) => item.provider_id === current) ? current : null
      );
      setProviderStatus(items.length === 0 ? "暂无 Provider，请先新增。" : "Provider 列表已更新。");
    } catch (error) {
      setProviderStatus((error as Error).message);
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

  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.provider_id === selectedProviderId) ?? null,
    [providers, selectedProviderId]
  );

  const providerFormInitialValue: ProviderFormValue = selectedProvider
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

  if (!props.open) {
    return null;
  }

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
    setProviderStatus("正在创建 Provider...");
    try {
      const created = await props.onCreateProvider(value);
      setProviders((prev) => [created, ...prev]);
      setSelectedProviderId(created.provider_id);
      setProviderStatus("Provider 创建成功。");
    } catch (error) {
      setProviderStatus((error as Error).message);
    } finally {
      setSavingProvider(false);
    }
  };

  const updateProvider = async (value: ProviderFormValue) => {
    if (!selectedProvider) {
      return;
    }
    setSavingProvider(true);
    setProviderStatus("正在保存 Provider...");
    try {
      const updated = await props.onUpdateProvider(selectedProvider.provider_id, value);
      setProviders((prev) =>
        prev.map((item) => (item.provider_id === updated.provider_id ? updated : item))
      );
      setProviderStatus("Provider 更新成功。");
    } catch (error) {
      setProviderStatus((error as Error).message);
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
    setProviderStatus("正在删除 Provider...");
    try {
      await props.onDeleteProvider(provider.provider_id);
      setProviders((prev) => prev.filter((item) => item.provider_id !== provider.provider_id));
      setSelectedProviderId((current) => (current === provider.provider_id ? null : current));
      setProviderStatus("Provider 已删除。");
    } catch (error) {
      setProviderStatus((error as Error).message);
    } finally {
      setSavingProvider(false);
    }
  };

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
              <section className="settings-panel provider-panel">
                <ProviderListPanel
                  providers={providers}
                  loading={loadingProviders}
                  selectedProviderId={selectedProviderId}
                  onRefresh={refreshProviders}
                  onSelect={(provider) => setSelectedProviderId(provider.provider_id)}
                  onEdit={(provider) => setSelectedProviderId(provider.provider_id)}
                  onDelete={removeProvider}
                />
                <div className="provider-form-wrap">
                  {providerStatus ? (
                    <div className="status-tip status-info" role="status" aria-live="polite">
                      {providerStatus}
                    </div>
                  ) : null}
                  <ProviderForm
                    mode={selectedProvider ? "edit" : "create"}
                    initialValue={providerFormInitialValue}
                    submitting={savingProvider}
                    onSubmit={selectedProvider ? updateProvider : createProvider}
                    onCancelEdit={() => setSelectedProviderId(null)}
                  />
                </div>
              </section>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
