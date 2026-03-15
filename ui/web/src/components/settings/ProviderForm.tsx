import { useEffect, useMemo, useState } from "react";

interface ProviderFormValue {
  providerName: string;
  baseUrl: string;
  modelName: string;
  apiKey: string;
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

  const normalizedInitialValue = useMemo(
    () => normalizeValue(props.initialValue),
    [props.initialValue]
  );
  const normalizedDraft = useMemo(() => normalizeValue(draft), [draft]);

  const isDirty =
    normalizedDraft.providerName !== normalizedInitialValue.providerName ||
    normalizedDraft.baseUrl !== normalizedInitialValue.baseUrl ||
    normalizedDraft.modelName !== normalizedInitialValue.modelName ||
    normalizedDraft.apiKey !== normalizedInitialValue.apiKey;

  const hasRequiredFields =
    normalizedDraft.providerName.length > 0 &&
    normalizedDraft.baseUrl.length > 0 &&
    normalizedDraft.modelName.length > 0 &&
    (props.mode === "edit" || normalizedDraft.apiKey.length > 0);

  return (
    <form
      className="provider-form"
      onSubmit={async (event) => {
        event.preventDefault();
        if (!hasRequiredFields || !isDirty || props.submitting) {
          return;
        }
        await props.onSubmit(normalizedDraft);
        if (props.mode === "create") {
          setDraft({
            providerName: "",
            baseUrl: "",
            modelName: "",
            apiKey: ""
          });
          setShowApiKey(false);
        }
      }}
    >
      <div className="provider-form-header">
        <h3>{props.mode === "create" ? "新增 Provider" : "编辑 Provider"}</h3>
        <p>{props.mode === "create" ? "填写信息后即可创建。" : "支持修改名称、地址和模型。"}</p>
      </div>

      <label>
        名称
        <input
          value={draft.providerName}
          placeholder="例如：openai-main"
          onChange={(event) => setDraft((prev) => ({ ...prev, providerName: event.target.value }))}
        />
      </label>
      <label>
        Base URL
        <input
          value={draft.baseUrl}
          placeholder="https://api.openai.com/v1"
          onChange={(event) => setDraft((prev) => ({ ...prev, baseUrl: event.target.value }))}
        />
      </label>
      <label>
        模型名称
        <input
          value={draft.modelName}
          placeholder="gpt-4.1-mini"
          onChange={(event) => setDraft((prev) => ({ ...prev, modelName: event.target.value }))}
        />
      </label>
      <label>
        API Key
        <div className="provider-key-row">
          <input
            type={showApiKey ? "text" : "password"}
            value={draft.apiKey}
            placeholder={props.mode === "edit" ? "留空则保持不变" : "输入 API Key"}
            onChange={(event) => setDraft((prev) => ({ ...prev, apiKey: event.target.value }))}
          />
          <button
            type="button"
            className="btn-ghost provider-key-toggle"
            aria-label={showApiKey ? "隐藏 API Key" : "显示 API Key"}
            onClick={() => setShowApiKey((prev) => !prev)}
          >
            {showApiKey ? "隐藏" : "显示"}
          </button>
        </div>
        {props.mode === "edit" ? <span className="field-tip">留空将保持现有 API Key。</span> : null}
      </label>

      <div className="provider-form-actions">
        {props.mode === "edit" ? (
          <button type="button" className="btn-secondary" onClick={props.onCancelEdit}>
            取消编辑
          </button>
        ) : null}
        <button
          className="btn-primary"
          type="submit"
          disabled={props.submitting || !hasRequiredFields || !isDirty}
        >
          {props.submitting
            ? props.mode === "create"
              ? "创建中..."
              : "保存中..."
            : props.mode === "create"
              ? "新增 Provider"
              : "保存修改"}
        </button>
      </div>
    </form>
  );
}

export type { ProviderFormValue };
