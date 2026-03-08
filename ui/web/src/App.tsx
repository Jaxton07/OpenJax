import { useState } from "react";
import ApprovalPanel from "./components/ApprovalPanel";
import Composer from "./components/Composer";
import MessageList from "./components/MessageList";
import SettingsModal from "./components/SettingsModal";
import Sidebar from "./components/Sidebar";
import { useChatApp } from "./hooks/useChatApp";
import { GatewayClient } from "./lib/gatewayClient";
import type { AppSettings } from "./types/gateway";

export default function App() {
  const {
    state,
    activeSession,
    newChat,
    switchSession,
    sendMessage,
    resolveApproval,
    clearConversation,
    compactConversation,
    updateSettings,
    dismissGlobalError,
    dismissToast
  } = useChatApp();

  const [settingsOpen, setSettingsOpen] = useState(false);

  const testConnection = async (settings: AppSettings) => {
    const client = new GatewayClient(settings);
    try {
      const result = await client.healthCheck();
      return result.status === "ok";
    } catch {
      return false;
    }
  };

  return (
    <div className="app-shell">
      <Sidebar
        sessions={state.sessions}
        activeSessionId={state.activeSessionId}
        onNewChat={() => void newChat()}
        onSelectSession={switchSession}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      <main className="chat-main">
        <header className="chat-header">
          <h1>OpenJax</h1>
          <div className="chat-status">{activeSession?.connection ?? "idle"}</div>
        </header>

        {state.globalError ? (
          <div className="banner error" onClick={dismissGlobalError}>
            {state.globalError}
          </div>
        ) : null}
        {state.infoToast ? (
          <div className="banner info" onClick={dismissToast}>
            {state.infoToast}
          </div>
        ) : null}

        <ApprovalPanel
          approvals={activeSession?.pendingApprovals ?? []}
          onResolve={(approval, approved) => resolveApproval(approval, approved)}
        />

        <MessageList messages={activeSession?.messages ?? []} />

        <Composer
          disabled={state.loading}
          onSend={sendMessage}
          onClear={() => void clearConversation()}
          onCompact={() => void compactConversation()}
        />
      </main>

      <SettingsModal
        open={settingsOpen}
        initial={state.settings}
        onClose={() => setSettingsOpen(false)}
        onSave={updateSettings}
        onTest={testConnection}
      />
    </div>
  );
}
