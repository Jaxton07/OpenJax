import { useState } from "react";
import type { CatalogProvider, LlmProvider } from "../../types/gateway";
import ProviderCard from "./ProviderCard";

interface ProviderListPanelProps {
  providers: LlmProvider[];
  loading: boolean;
  selectedProviderId: string | null;
  catalog: CatalogProvider[];
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
        <div className="provider-list-actions">
          <button type="button" className="btn-secondary" onClick={props.onAddProvider}>
            Add Provider
          </button>
          <button type="button" className="btn-secondary" onClick={() => void props.onRefresh()}>
            {props.loading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </div>

      <div className="provider-list-scroll">
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
              <ProviderCard
                key={provider.provider_id}
                provider={provider}
                isActive={isActive}
                isSwitching={isSwitching}
                catalogEntry={catalogEntry}
                onSelect={() => props.onSelect(provider)}
                onEdit={() => props.onEdit(provider)}
                onDelete={() => void props.onDelete(provider)}
                onModelSwitch={(modelId) => void handleModelSwitch(provider, modelId)}
              />
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
      </div>
    </section>
  );
}
