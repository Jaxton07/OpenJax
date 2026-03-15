import type { AppSettings, OutputMode } from "../../types/gateway";

interface GeneralSettingsPanelProps {
  draft: AppSettings;
  status: string;
  onChangeDraft: (next: AppSettings) => void;
  onTest: (settings: AppSettings) => Promise<boolean>;
  onSave: (settings: AppSettings) => void;
}

export default function GeneralSettingsPanel(props: GeneralSettingsPanelProps) {
  const updateMode = (outputMode: OutputMode) => {
    props.onChangeDraft({ ...props.draft, outputMode });
  };

  return (
    <section className="settings-panel">
      <label>
        Gateway Base URL
        <input
          value={props.draft.baseUrl}
          onChange={(event) => props.onChangeDraft({ ...props.draft, baseUrl: event.target.value })}
        />
      </label>

      <div className="output-mode-group">
        <span>输出模式</span>
        <div>
          <button
            className={props.draft.outputMode === "sse" ? "active" : ""}
            onClick={() => updateMode("sse")}
          >
            SSE（默认）
          </button>
          <button
            className={props.draft.outputMode === "polling" ? "active" : ""}
            onClick={() => updateMode("polling")}
          >
            Polling（备用）
          </button>
        </div>
      </div>

      <div className="modal-actions">
        <button
          onClick={async () => {
            await props.onTest(props.draft);
          }}
        >
          测试连接
        </button>
        <button
          className="primary"
          onClick={() => {
            props.onSave(props.draft);
          }}
        >
          保存
        </button>
      </div>

      {props.status ? <div className="status-tip">{props.status}</div> : null}
    </section>
  );
}
