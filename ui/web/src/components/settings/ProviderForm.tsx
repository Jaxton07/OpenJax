import { useEffect, useMemo, useState } from "react";
import type { CatalogModel } from "../../types/gateway";

interface ProviderFormValue {
  providerName: string;
  baseUrl: string;
  modelName: string;
  apiKey: string;
  providerType: "built_in" | "custom";
  contextWindowSize: number;
  catalogModels?: CatalogModel[];
}

interface ProviderFormProps {
  mode: "create" | "edit";
  initialValue: ProviderFormValue;
  submitting: boolean;
  onSubmit: (value: ProviderFormValue) => Promise<void>;
  onCancelEdit?: () => void;
}

function normalizeValue(value: ProviderFormValue): ProviderFormValue {
  return {
    ...value,
    providerName: value.providerName.trim(),
    baseUrl: value.baseUrl.trim(),
    modelName: value.modelName.trim(),
    apiKey: value.apiKey.trim()
  };
}

export default function ProviderForm(props: ProviderFormProps) {
  const [draft, setDraft] = useState<ProviderFormValue>(props.initialValue);
  const [showApiKey, setShowApiKey] = useState(false);

  useEffect(() => {
    setDraft(props.initialValue);
    setShowApiKey(false);
  }, [props.initialValue]);

  const normalizedInitial = useMemo(() => normalizeValue(props.initialValue), [props.initialValue]);
  const normalizedDraft = useMemo(() => normalizeValue(draft), [draft]);

  const isDirty =
    normalizedDraft.providerName !== normalizedInitial.providerName ||
    normalizedDraft.baseUrl !== normalizedInitial.baseUrl ||
    normalizedDraft.modelName !== normalizedInitial.modelName ||
    normalizedDraft.apiKey !== normalizedInitial.apiKey ||
    normalizedDraft.contextWindowSize !== normalizedInitial.contextWindowSize;

  const isBuiltIn = draft.providerType === "built_in";

  const hasRequiredFields = isBuiltIn
    ? normalizedDraft.apiKey.length > 0 || props.mode === "edit"
    : normalizedDraft.providerName.length > 0 &&
      normalizedDraft.baseUrl.length > 0 &&
      normalizedDraft.modelName.length > 0 &&
      normalizedDraft.contextWindowSize > 0 &&
      (props.mode === "edit" || normalizedDraft.apiKey.length > 0);

  const formatContextWindow = (n: number) =>
    n > 0 ? `${n.toLocaleString()} tokens` : "—";

  return (
    <form
      className="provider-form"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!hasRequiredFields || !isDirty || props.submitting) return;
        await props.onSubmit(normalizedDraft);
        if (props.mode === "create") {
          setDraft({ ...props.initialValue, apiKey: "" });
          setShowApiKey(false);
        }
      }}
    >
      <div className="provider-form-header">
        <h3>{props.mode === "create" ? "新增 Provider" : "编辑 Provider"}</h3>
        <p>
          {isBuiltIn
            ? "内置 Provider，只需填写 API Key。"
            : props.mode === "create"
            ? "填写信息后即可创建。"
            : "支持修改名称、地址和模型。"}
        </p>
      </div>

      {/* 名称 */}
      <label>
        名称
        {isBuiltIn ? (
          <input value={draft.providerName} readOnly className="field-readonly" />
        ) : (
          <input
            value={draft.providerName}
            placeholder="例如：openai-main"
            onChange={(e) => setDraft((p) => ({ ...p, providerName: e.target.value }))}
          />
        )}
      </label>

      {/* Base URL */}
      <label>
        Base URL
        {isBuiltIn ? (
          <input value={draft.baseUrl} readOnly className="field-readonly" />
        ) : (
          <input
            value={draft.baseUrl}
            placeholder="https://api.openai.com/v1"
            onChange={(e) => setDraft((p) => ({ ...p, baseUrl: e.target.value }))}
          />
        )}
      </label>

      {/* 模型名称 */}
      <label>
        模型名称
        {isBuiltIn && draft.catalogModels && draft.catalogModels.length > 0 ? (
          <select
            value={draft.modelName}
            onChange={(e) => {
              const selected = draft.catalogModels!.find((m) => m.model_id === e.target.value);
              setDraft((p) => ({
                ...p,
                modelName: e.target.value,
                contextWindowSize: selected?.context_window ?? p.contextWindowSize
              }));
            }}
          >
            {draft.catalogModels.map((m) => (
              <option key={m.model_id} value={m.model_id}>
                {m.display_name} · {(m.context_window / 1000).toFixed(0)}k
              </option>
            ))}
          </select>
        ) : (
          <input
            value={draft.modelName}
            placeholder="gpt-4o"
            readOnly={isBuiltIn}
            className={isBuiltIn ? "field-readonly" : undefined}
            onChange={(e) => setDraft((p) => ({ ...p, modelName: e.target.value }))}
          />
        )}
      </label>

      {/* 上下文窗口大小 */}
      <label>
        上下文窗口大小
        {isBuiltIn ? (
          <input value={formatContextWindow(draft.contextWindowSize)} readOnly className="field-readonly" />
        ) : (
          <input
            type="number"
            value={draft.contextWindowSize || ""}
            placeholder="如 128000"
            min={1}
            onChange={(e) =>
              setDraft((p) => ({ ...p, contextWindowSize: parseInt(e.target.value, 10) || 0 }))
            }
          />
        )}
      </label>

      {/* API Key */}
      <label>
        API Key
        <div className="provider-key-row">
          <input
            type={showApiKey ? "text" : "password"}
            value={draft.apiKey}
            placeholder={props.mode === "edit" ? "留空则保持不变" : "输入 API Key"}
            onChange={(e) => setDraft((p) => ({ ...p, apiKey: e.target.value }))}
          />
          <button
            type="button"
            className="btn-ghost provider-key-toggle"
            onClick={() => setShowApiKey((v) => !v)}
          >
            {showApiKey ? "隐藏" : "显示"}
          </button>
        </div>
        {props.mode === "edit" && <span className="field-tip">留空将保持现有 API Key。</span>}
      </label>

      <div className="provider-form-actions">
        {props.mode === "edit" && (
          <button type="button" className="btn-secondary" onClick={props.onCancelEdit}>
            取消编辑
          </button>
        )}
        <button
          className="btn-primary"
          type="submit"
          disabled={props.submitting || !hasRequiredFields || !isDirty}
        >
          {props.submitting
            ? props.mode === "create" ? "创建中..." : "保存中..."
            : props.mode === "create" ? "新增 Provider" : "保存修改"}
        </button>
      </div>
    </form>
  );
}

export type { ProviderFormValue };
