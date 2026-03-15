import { useEffect, useState } from "react";
import type { AppSettings, LlmProvider } from "../types/gateway";
import GeneralSettingsPanel from "./settings/GeneralSettingsPanel";
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

export default function SettingsModal(props: SettingsModalProps) {
  const [draft, setDraft] = useState<AppSettings>(props.initialSettings);
  const [status, setStatus] = useState<string>("");
  const [activeTab, setActiveTab] = useState<"general" | "provider">("general");
  const [providers, setProviders] = useState<LlmProvider[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(false);
  const [savingProvider, setSavingProvider] = useState(false);
  const [editingProvider, setEditingProvider] = useState<LlmProvider | null>(null);

  useEffect(() => {
    if (props.open) {
      setDraft(props.initialSettings);
      setStatus("");
      setActiveTab("general");
      setEditingProvider(null);
    }
  }, [props.initialSettings, props.open]);

  useEffect(() => {
    if (!props.open || activeTab !== "provider") {
      return;
    }
    void (async () => {
      setLoadingProviders(true);
      try {
        const items = await props.onListProviders();
        setProviders(items);
      } catch (error) {
        setStatus((error as Error).message);
      } finally {
        setLoadingProviders(false);
      }
    })();
  }, [activeTab, props.open, props.onListProviders]);

  if (!props.open) {
    return null;
  }

  const handleSaveSettings = (settings: AppSettings) => {
    props.onSave(settings);
    setStatus("设置已保存");
    props.onClose();
  };

  const handleTestSettings = async (settings: AppSettings) => {
    const ok = await props.onTest(settings);
    setStatus(ok ? "连接测试成功" : "连接测试失败");
    return ok;
  };

  const createProvider = async (value: ProviderFormValue) => {
    setSavingProvider(true);
    try {
      const created = await props.onCreateProvider(value);
      setProviders((prev) => [created, ...prev]);
      setStatus("Provider 创建成功");
    } finally {
      setSavingProvider(false);
    }
  };

  const updateProvider = async (value: ProviderFormValue) => {
    if (!editingProvider) {
      return;
    }
    setSavingProvider(true);
    try {
      const updated = await props.onUpdateProvider(editingProvider.provider_id, value);
      setProviders((prev) =>
        prev.map((item) => (item.provider_id === updated.provider_id ? updated : item))
      );
      setEditingProvider(null);
      setStatus("Provider 更新成功");
    } finally {
      setSavingProvider(false);
    }
  };

  const removeProvider = async (provider: LlmProvider) => {
    const confirmed = window.confirm(`确认删除 Provider「${provider.provider_name}」？`);
    if (!confirmed) {
      return;
    }
    await props.onDeleteProvider(provider.provider_id);
    setProviders((prev) => prev.filter((item) => item.provider_id !== provider.provider_id));
    setStatus("Provider 已删除");
  };

  const providerFormInitialValue: ProviderFormValue = editingProvider
    ? {
        providerName: editingProvider.provider_name,
        baseUrl: editingProvider.base_url,
        modelName: editingProvider.model_name,
        apiKey: ""
      }
    : {
        providerName: "",
        baseUrl: "",
        modelName: "",
        apiKey: ""
      };

  return (
    <div className="modal-backdrop" onClick={props.onClose}>
      <div className="settings-modal" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <h2>设置</h2>
          <button onClick={props.onClose}>关闭</button>
        </div>
        <div className="settings-modal-body">
          <SettingsSidebar activeTab={activeTab} onChangeTab={setActiveTab} />
          <div className="settings-modal-content">
            {activeTab === "general" ? (
              <GeneralSettingsPanel
                draft={draft}
                status={status}
                onChangeDraft={setDraft}
                onTest={handleTestSettings}
                onSave={handleSaveSettings}
              />
            ) : (
              <section className="settings-panel provider-panel">
                <ProviderListPanel
                  providers={providers}
                  loading={loadingProviders}
                  onRefresh={async () => {
                    const items = await props.onListProviders();
                    setProviders(items);
                  }}
                  onEdit={setEditingProvider}
                  onDelete={removeProvider}
                />
                <ProviderForm
                  mode={editingProvider ? "edit" : "create"}
                  initialValue={providerFormInitialValue}
                  submitting={savingProvider}
                  onSubmit={editingProvider ? updateProvider : createProvider}
                  onCancelEdit={() => setEditingProvider(null)}
                />
              </section>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
