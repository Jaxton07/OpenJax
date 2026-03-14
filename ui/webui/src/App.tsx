import Composer from "./components/Composer";
import ConversationPane from "./components/ConversationPane";
import ConnectionPanel from "./components/ConnectionPanel";
import { useChatPage } from "./hooks/useChatPage";

export default function App() {
  const {
    state,
    setBaseUrl,
    setOwnerKey,
    connect,
    send,
    rebuildSession,
    dismissError,
    dismissInfo
  } = useChatPage();

  return (
    <div className="app-root">
      <aside className="left-rail">
        <header className="topbar">
          <h1>OpenJax WebUI v2</h1>
          <div className="status-group">
            <span className={`status-dot ${state.connected ? "ok" : "idle"}`} />
            <span>{state.connected ? "connected" : "disconnected"}</span>
          </div>
        </header>
        {state.sessionId ? <code className="session-id">{state.sessionId}</code> : null}
        <ConnectionPanel
          baseUrl={state.baseUrl}
          ownerKey={state.ownerKey}
          connected={state.connected}
          onBaseUrlChange={setBaseUrl}
          onOwnerKeyChange={setOwnerKey}
          onConnect={() => void connect()}
        />
      </aside>

      <main className="right-main">
        {state.globalError ? (
          <div className="banner error" onClick={dismissError}>
            {state.globalError}
          </div>
        ) : null}

        {state.info ? (
          <div className="banner info" onClick={dismissInfo}>
            {state.info}
          </div>
        ) : null}

        <ConversationPane
          sessionId={state.sessionId ?? undefined}
          turnId={state.activeTurnId ?? undefined}
          users={state.users}
          assistants={state.assistants}
        />

        <Composer
          disabled={!state.connected || state.sending || state.replayExceeded}
          onSend={send}
        />

        {state.replayExceeded ? (
          <div className="recovery-box">
            <span>当前流回放窗口超限，需重建会话。</span>
            <button className="primary" onClick={() => void rebuildSession()}>
              Rebuild Session
            </button>
          </div>
        ) : null}
      </main>
    </div>
  );
}
