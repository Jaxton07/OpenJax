import { useState } from "react";
import type { CatalogProvider, LlmProvider } from "../../types/gateway";
import { TrashIcon } from "../../pic/icon";

const EDIT_ICON = new URL("../../pic/icon/edit.svg", import.meta.url).href;

interface ProviderListPanelProps {
  providers: LlmProvider[];
  loading: boolean;
  selectedProviderId: string | null;
  catalog: CatalogProvider[];
  noticeMessage?: string;
  noticeTone?: "success" | "error" | "info";
  onRefresh: () => Promise<void>;
  onAddProvider: () => void;
  onSelect: (provider: LlmProvider) => void;
  onEdit: (provider: LlmProvider) => void;
  onDelete: (provider: LlmProvider) => Promise<void>;
  onSwitchModel: (providerId: string, modelId: string, contextWindow: number) => Promise<void>;
  onConfigureCatalogEntry: (entry: CatalogProvider) => void;
}

function normalizeUrl(url: string) {
  return url.replace(/\/+$/, "");
}

export default function ProviderListPanel(props: ProviderListPanelProps) {
  const [switchingId, setSwitchingId] = useState<string | null>(null);

  // 激活的 provider 置顶
  const sortedProviders = [...props.providers].sort((a, b) => {
    if (a.provider_id === props.selectedProviderId) return -1;
    if (b.provider_id === props.selectedProviderId) return 1;
    return 0;
  });

  // 目录中尚未配置的条目（按 base_url 匹配已配置的内置 provider）
  const unconfiguredCatalog = props.catalog.filter(
    (entry) =>
      !props.providers.some(
        (p) =>
          p.provider_type === "built_in" &&
          normalizeUrl(p.base_url) === normalizeUrl(entry.base_url)
      )
  );

  const handleModelSwitch = async (provider: LlmProvider, modelId: string) => {
    const catalogEntry = props.catalog.find(
      (e) => normalizeUrl(e.base_url) === normalizeUrl(provider.base_url)
    );
    const model = catalogEntry?.models.find((m) => m.model_id === modelId);
    if (!model) return;
    setSwitchingId(provider.provider_id);
    try {
      await props.onSwitchModel(provider.provider_id, modelId, model.context_window);
    } finally {
      setTimeout(() => setSwitchingId(null), 2000);
    }
  };

  return (
    <section className="provider-list-panel">
      <div className="provider-list-header">
        <div className="provider-list-title">
          <h3>Provider List</h3>
        </div>
        <div className="provider-list-notice-slot" aria-live="polite">
          {props.noticeMessage ? (
            <div className={`provider-inline-notice provider-inline-notice-${props.noticeTone ?? "info"}`}>
              {props.noticeMessage}
            </div>
          ) : null}
        </div>
        <div className="provider-list-actions">
          <button type="button" className="btn-secondary" onClick={props.onAddProvider}>
            Add Provider
          </button>
          <button type="button" className="btn-secondary" onClick={() => void props.onRefresh()}>
            {props.loading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </div>

      {props.loading && props.providers.length === 0 ? (
        <div className="status-tip status-info">正在加载 Provider...</div>
      ) : null}

      {/* 已配置区 */}
      <ul className="provider-list">
        {sortedProviders.map((provider) => {
          const isActive = props.selectedProviderId === provider.provider_id;
          const isSwitching = switchingId === provider.provider_id;
          const catalogEntry =
            provider.provider_type === "built_in"
              ? props.catalog.find(
                  (e) => normalizeUrl(e.base_url) === normalizeUrl(provider.base_url)
                )
              : undefined;

          return (
            <li key={provider.provider_id} className={isActive ? "provider-row selected" : "provider-row"}>
              <button
                type="button"
                className="provider-card"
                aria-pressed={isActive}
                onClick={() => props.onSelect(provider)}
              >
                <div className="provider-card-main">
                  <strong>
                    {provider.provider_name}
                    {provider.provider_type === "built_in" && (
                      <span className="provider-builtin-badge">内置</span>
                    )}
                  </strong>
                  {/* 内置 provider：模型下拉 */}
                  {catalogEntry ? (
                    <div
                      className="provider-model-switch"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <select
                        value={provider.model_name}
                        onChange={(e) => void handleModelSwitch(provider, e.target.value)}
                        disabled={isSwitching}
                      >
                        {catalogEntry.models.map((m) => (
                          <option key={m.model_id} value={m.model_id}>
                            {m.display_name} · {(m.context_window / 1000).toFixed(0)}k
                          </option>
                        ))}
                      </select>
                      {isSwitching && <span className="provider-switch-tip">已切换</span>}
                    </div>
                  ) : (
                    <span>{provider.model_name}</span>
                  )}
                  <span>{provider.base_url}</span>
                  <span className="provider-cw">
                    {provider.context_window_size > 0
                      ? `${(provider.context_window_size / 1000).toFixed(0)}k ctx`
                      : null}
                  </span>
                  <span>{provider.api_key_set ? "API Key 已设置" : "API Key 未设置"}</span>
                </div>
              </button>
              <div className="provider-card-actions">
                <button
                  type="button"
                  className="provider-action-btn"
                  aria-label="编辑"
                  title="编辑"
                  onClick={() => props.onEdit(provider)}
                >
                  <img src={EDIT_ICON} alt="" />
                </button>
                <button
                  type="button"
                  className="provider-action-btn danger"
                  aria-label="删除"
                  title="删除"
                  onClick={() => void props.onDelete(provider)}
                >
                  <TrashIcon />
                </button>
              </div>
            </li>
          );
        })}
      </ul>

      {props.providers.length === 0 && !props.loading ? (
        <div className="status-tip status-info">暂无 Provider，请先新增。</div>
      ) : null}

      {/* 可添加的内置目录区 */}
      {unconfiguredCatalog.length > 0 && (
        <div className="catalog-section">
          <div className="catalog-section-label">可添加的内置 Provider</div>
          <ul className="catalog-list">
            {unconfiguredCatalog.map((entry) => (
              <li key={entry.catalog_key} className="catalog-row">
                <div className="catalog-info">
                  <strong>{entry.display_name}</strong>
                  <span>
                    {entry.default_model} ·{" "}
                    {(() => {
                      const m = entry.models.find((m) => m.model_id === entry.default_model);
                      return m ? `${(m.context_window / 1000).toFixed(0)}k` : "";
                    })()}
                  </span>
                </div>
                <button
                  type="button"
                  className="btn-secondary catalog-add-btn"
                  onClick={() => props.onConfigureCatalogEntry(entry)}
                >
                  + 配置
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
