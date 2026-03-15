import type { LlmProvider } from "../../types/gateway";

interface ProviderListPanelProps {
  providers: LlmProvider[];
  loading: boolean;
  onRefresh: () => Promise<void>;
  onEdit: (provider: LlmProvider) => void;
  onDelete: (provider: LlmProvider) => Promise<void>;
}

export default function ProviderListPanel(props: ProviderListPanelProps) {
  return (
    <section className="provider-list-panel">
      <div className="provider-list-header">
        <h3>已有 Provider</h3>
        <button onClick={() => void props.onRefresh()} disabled={props.loading}>
          刷新
        </button>
      </div>
      <ul className="provider-list">
        {props.providers.map((provider) => (
          <li key={provider.provider_id}>
            <div className="provider-card-main">
              <strong>{provider.provider_name}</strong>
              <span>{provider.model_name}</span>
              <span>{provider.base_url}</span>
              <span>{provider.api_key_set ? "API Key 已设置" : "API Key 未设置"}</span>
            </div>
            <div className="provider-card-actions">
              <button onClick={() => props.onEdit(provider)}>编辑</button>
              <button onClick={() => void props.onDelete(provider)}>删除</button>
            </div>
          </li>
        ))}
      </ul>
      {props.providers.length === 0 ? <div className="status-tip">暂无 Provider，请先新增。</div> : null}
    </section>
  );
}
