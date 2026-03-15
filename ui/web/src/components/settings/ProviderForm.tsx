import { useEffect, useState } from "react";

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

export default function ProviderForm(props: ProviderFormProps) {
  const [draft, setDraft] = useState<ProviderFormValue>(props.initialValue);

  useEffect(() => {
    setDraft(props.initialValue);
  }, [props.initialValue]);

  return (
    <form
      className="provider-form"
      onSubmit={async (event) => {
        event.preventDefault();
        await props.onSubmit({
          providerName: draft.providerName.trim(),
          baseUrl: draft.baseUrl.trim(),
          modelName: draft.modelName.trim(),
          apiKey: draft.apiKey.trim()
        });
        if (props.mode === "create") {
          setDraft({
            providerName: "",
            baseUrl: "",
            modelName: "",
            apiKey: ""
          });
        }
      }}
    >
      <label>
        名称
        <input
          value={draft.providerName}
          onChange={(event) => setDraft((prev) => ({ ...prev, providerName: event.target.value }))}
        />
      </label>
      <label>
        Base URL
        <input
          value={draft.baseUrl}
          onChange={(event) => setDraft((prev) => ({ ...prev, baseUrl: event.target.value }))}
        />
      </label>
      <label>
        模型名称
        <input
          value={draft.modelName}
          onChange={(event) => setDraft((prev) => ({ ...prev, modelName: event.target.value }))}
        />
      </label>
      <label>
        API Key
        <input
          value={draft.apiKey}
          placeholder={props.mode === "edit" ? "留空则保持不变" : ""}
          onChange={(event) => setDraft((prev) => ({ ...prev, apiKey: event.target.value }))}
        />
      </label>
      <div className="provider-form-actions">
        {props.mode === "edit" ? (
          <button type="button" onClick={props.onCancelEdit}>
            取消
          </button>
        ) : null}
        <button className="primary" type="submit" disabled={props.submitting}>
          {props.mode === "create" ? "新增 Provider" : "保存修改"}
        </button>
      </div>
    </form>
  );
}

export type { ProviderFormValue };
