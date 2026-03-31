import type { CatalogProvider, LlmProvider } from "../../types/gateway";
import { EditIcon, TrashIcon } from "../../pic/icon";

interface ProviderCardProps {
  provider: LlmProvider;
  isActive: boolean;
  isSwitching: boolean;
  catalogEntry?: CatalogProvider;
  onSelect: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onModelSwitch: (modelId: string) => void;
}

export default function ProviderCard({
  provider,
  isActive,
  isSwitching,
  catalogEntry,
  onSelect,
  onEdit,
  onDelete,
  onModelSwitch,
}: ProviderCardProps) {
  const row2Parts = provider.base_url;

  return (
    <li className={isActive ? "provider-row selected" : "provider-row"}>
      <button
        type="button"
        className="provider-card"
        aria-pressed={isActive}
        onClick={onSelect}
      >
        <div className="provider-card-main">
          {/* 行 1：名称 + badge + 模型切换器 */}
          <div className="provider-card-row1">
            <strong>
              {provider.provider_name}
              {provider.provider_type === "built_in" && (
                <span className="provider-builtin-badge">内置</span>
              )}
            </strong>
            {catalogEntry ? (
              <div
                className="provider-model-switch"
                onClick={(e) => e.stopPropagation()}
              >
                <select
                  value={provider.model_name}
                  onChange={(e) => onModelSwitch(e.target.value)}
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
              <span className="provider-model-name">{provider.model_name}</span>
            )}
          </div>
          {/* 行 2：URL · ctx */}
          {row2Parts ? (
            <div className="provider-card-row2">{row2Parts}</div>
          ) : null}
        </div>
      </button>
      <div className="provider-card-actions">
        <button
          type="button"
          className="provider-action-btn"
          aria-label="编辑"
          title="编辑"
          onClick={onEdit}
        >
          <EditIcon />
        </button>
        <button
          type="button"
          className="provider-action-btn danger"
          aria-label="删除"
          title="删除"
          onClick={onDelete}
        >
          <TrashIcon />
        </button>
      </div>
    </li>
  );
}
