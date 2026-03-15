import type { LlmProvider } from "../../types/gateway";
import { TrashIcon } from "../../pic/icon";

const EDIT_ICON = new URL("../../pic/icon/edit.svg", import.meta.url).href;

interface ProviderListPanelProps {
  providers: LlmProvider[];
  loading: boolean;
  selectedProviderId: string | null;
  noticeMessage?: string;
  noticeTone?: "success" | "error" | "info";
  onRefresh: () => Promise<void>;
  onAddProvider: () => void;
  onSelect: (provider: LlmProvider) => void;
  onEdit: (provider: LlmProvider) => void;
  onDelete: (provider: LlmProvider) => Promise<void>;
}

export default function ProviderListPanel(props: ProviderListPanelProps) {
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

      <ul className="provider-list">
        {props.providers.map((provider) => {
          const selected = props.selectedProviderId === provider.provider_id;
          return (
            <li key={provider.provider_id} className={selected ? "provider-row selected" : "provider-row"}>
              <button
                type="button"
                className="provider-card"
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
    </section>
  );
}
