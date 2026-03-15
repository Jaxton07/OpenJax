import type { AppSettings, OutputMode } from "../../types/gateway";

interface GeneralStatus {
  tone: "idle" | "info" | "success" | "error";
  message: string;
}

interface GeneralSettingsPanelProps {
  draft: AppSettings;
  status: GeneralStatus;
  testing: boolean;
  saving: boolean;
  onChangeDraft: (next: AppSettings) => void;
  onTest: (settings: AppSettings) => Promise<void>;
  onSave: (settings: AppSettings) => Promise<void>;
}

export default function GeneralSettingsPanel(props: GeneralSettingsPanelProps) {
  const updateMode = (outputMode: OutputMode) => {
    props.onChangeDraft({ ...props.draft, outputMode });
  };

  return (
    <section className="settings-panel">
      <header className="settings-section-header">
        <h3>通用设置</h3>
        <p>调整网关连接和输出方式。</p>
      </header>

      <div className="settings-row">
        <div className="settings-row-meta">
          <label htmlFor="gateway-base-url">Gateway Base URL</label>
          <p>用于连接 OpenJax Gateway 服务。</p>
        </div>
        <div className="settings-row-control">
          <input
            id="gateway-base-url"
            value={props.draft.baseUrl}
            placeholder="http://127.0.0.1:8765"
            onChange={(event) => props.onChangeDraft({ ...props.draft, baseUrl: event.target.value })}
          />
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-meta">
          <span>输出模式</span>
          <p>SSE 为默认模式，Polling 可作为网络不稳定时的备用方案。</p>
        </div>
        <div className="settings-row-control">
          <div className="output-mode-group" role="radiogroup" aria-label="输出模式">
            <button
              type="button"
              role="radio"
              aria-checked={props.draft.outputMode === "sse"}
              className={props.draft.outputMode === "sse" ? "active" : ""}
              onClick={() => updateMode("sse")}
            >
              SSE（默认）
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={props.draft.outputMode === "polling"}
              className={props.draft.outputMode === "polling" ? "active" : ""}
              onClick={() => updateMode("polling")}
            >
              Polling（备用）
            </button>
          </div>
        </div>
      </div>

      <footer className="settings-action-bar">
        <div
          className={`status-tip status-${props.status.tone}`}
          role={props.status.tone === "error" ? "alert" : "status"}
          aria-live="polite"
        >
          {props.status.message}
        </div>
        <div className="settings-action-group">
          <button
            type="button"
            className="btn-secondary"
            onClick={() => void props.onTest(props.draft)}
            disabled={props.testing || props.saving}
          >
            {props.testing ? "测试中..." : "测试连接"}
          </button>
          <button
            type="button"
            className="btn-primary"
            onClick={() => void props.onSave(props.draft)}
            disabled={props.testing || props.saving}
          >
            {props.saving ? "保存中..." : "保存"}
          </button>
        </div>
      </footer>
    </section>
  );
}

export type { GeneralStatus };
