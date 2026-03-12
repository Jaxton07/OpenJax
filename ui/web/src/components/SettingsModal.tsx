import { useEffect, useState } from "react";
import type { AppSettings, OutputMode } from "../types/gateway";

interface SettingsModalProps {
  open: boolean;
  initialSettings: AppSettings;
  initialApiKey: string;
  onClose: () => void;
  onSave: (settings: AppSettings, apiKey: string) => void;
  onTest: (settings: AppSettings, apiKey: string) => Promise<boolean>;
}

export default function SettingsModal(props: SettingsModalProps) {
  const [draft, setDraft] = useState<AppSettings>(props.initialSettings);
  const [apiKey, setApiKey] = useState(props.initialApiKey);
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    if (props.open) {
      setDraft(props.initialSettings);
      setApiKey(props.initialApiKey);
      setStatus("");
    }
  }, [props.initialApiKey, props.initialSettings, props.open]);

  if (!props.open) {
    return null;
  }

  const updateMode = (outputMode: OutputMode) => {
    setDraft((prev) => ({ ...prev, outputMode }));
  };

  return (
    <div className="modal-backdrop" onClick={props.onClose}>
      <div className="settings-modal" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <h2>设置</h2>
          <button onClick={props.onClose}>关闭</button>
        </div>

        <label>
          Gateway Base URL
          <input
            value={draft.baseUrl}
            onChange={(event) => setDraft((prev) => ({ ...prev, baseUrl: event.target.value }))}
          />
        </label>

        <label>
          Access Key
          <input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
          />
        </label>

        <div className="output-mode-group">
          <span>输出模式</span>
          <div>
            <button
              className={draft.outputMode === "sse" ? "active" : ""}
              onClick={() => updateMode("sse")}
            >
              SSE（默认）
            </button>
            <button
              className={draft.outputMode === "polling" ? "active" : ""}
              onClick={() => updateMode("polling")}
            >
              Polling（备用）
            </button>
          </div>
        </div>

        <div className="modal-actions">
          <button
            onClick={async () => {
              const ok = await props.onTest(draft, apiKey);
              setStatus(ok ? "连接测试成功" : "连接测试失败");
            }}
          >
            测试连接
          </button>
          <button
            className="primary"
            onClick={() => {
              props.onSave(draft, apiKey);
              props.onClose();
            }}
          >
            保存
          </button>
        </div>

        {status ? <div className="status-tip">{status}</div> : null}
      </div>
    </div>
  );
}
