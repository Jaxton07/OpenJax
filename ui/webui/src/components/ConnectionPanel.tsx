interface ConnectionPanelProps {
  baseUrl: string;
  ownerKey: string;
  connected: boolean;
  onBaseUrlChange: (value: string) => void;
  onOwnerKeyChange: (value: string) => void;
  onConnect: () => void;
}

export default function ConnectionPanel(props: ConnectionPanelProps) {
  return (
    <section className="panel connection-panel">
      <h2>Gateway Connection</h2>
      <div className="field-grid">
        <label>
          Base URL
          <input
            value={props.baseUrl}
            onChange={(event) => props.onBaseUrlChange(event.target.value)}
            placeholder="http://127.0.0.1:8765"
          />
        </label>

        <label>
          Owner Key
          <input
            type="password"
            value={props.ownerKey}
            onChange={(event) => props.onOwnerKeyChange(event.target.value)}
            placeholder="ojx_xxxxxxxxx"
          />
        </label>
      </div>

      <button className="primary" onClick={props.onConnect}>
        {props.connected ? "Reconnect" : "Connect"}
      </button>
    </section>
  );
}
