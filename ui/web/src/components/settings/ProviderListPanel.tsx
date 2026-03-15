import type { LlmProvider } from "../../types/gateway";

interface ProviderListPanelProps {
  providers: LlmProvider[];
  loading: boolean;
  selectedProviderId: string | null;
  onRefresh: () => Promise<void>;
  onSelect: (provider: LlmProvider) => void;
  onEdit: (provider: LlmProvider) => void;
  onDelete: (provider: LlmProvider) => Promise<void>;
}

export default function ProviderListPanel(props: ProviderListPanelProps) {
  return (
    <section className="provider-list-panel">
      <div className="provider-list-header">
        <div>
          <h3>已有 Provider</h3>
          <p>选择一项可在右侧编辑；不选择时右侧为新增模式。</p>
        </div>
        <button type="button" className="btn-secondary" onClick={() => void props.onRefresh()}>
          {props.loading ? "刷新中..." : "刷新"}
        </button>
      </div>

      {props.loading && props.providers.length === 0 ? (
        <div className="status-tip status-info">正在加载 Provider...</div>
      ) : null}

      <ul className="provider-list">
        {props.providers.map((provider) => {
          const selected = props.selectedProviderId === provider.provider_id;
          return (
            <li key={provider.provider_id}>
              <button
                type="button"
                className={selected ? "provider-card selected" : "provider-card"}
                aria-pressed={selected}
                onClick={() => props.onSelect(provider)}
              >
                <div className="provider-card-main">
                  <strong>{provider.provider_name}</strong>
                  <span>{provider.model_name}</span>
                  <span>{provider.base_url}</span>
                  <span>{provider.api_key_set ? "API Key 已设置" : "API Key 未设置"}</span>
                </div>
              </button>
              <div className="provider-card-actions">
                <button type="button" className="btn-ghost" onClick={() => props.onEdit(provider)}>
                  编辑
                </button>
                <button
                  type="button"
                  className="btn-danger"
                  onClick={() => void props.onDelete(provider)}
                >
                  删除
                </button>
              </div>
            </li>
          );
        })}
      </ul>

      {props.providers.length === 0 && !props.loading ? (
        <div className="status-tip status-info">暂无 Provider，请先在右侧新增。</div>
      ) : null}
    </section>
  );
}
