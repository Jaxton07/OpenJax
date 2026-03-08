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
    deleteSession,
    sendMessage,
    resolveApproval,
    clearConversation,
    compactConversation,
    updateSettings,
    dismissGlobalError,
    dismissToast
  } = useChatApp();

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(true);

  const testConnection = async (settings: AppSettings) => {
    const client = new GatewayClient(settings);
    try {
      await client.healthCheck();
      const created = await client.startSession();
      await client.shutdownSession(created.session_id);
      return true;
    } catch {
      return false;
    }
  };

  return (
    <div className={`app-shell ${sidebarCollapsed ? "sidebar-collapsed" : ""}`}>
      <Sidebar
        sessions={state.sessions}
        activeSessionId={state.activeSessionId}
        collapsed={sidebarCollapsed}
        onSelectSession={switchSession}
        onDeleteSession={deleteSession}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      <main className="chat-main">
        <header className="chat-header">
          <div className="chat-header-main">
            <button
              className="sidebar-toggle-btn"
              onClick={() => setSidebarCollapsed((prev) => !prev)}
              title={sidebarCollapsed ? "展开侧边栏" : "收起侧边栏"}
              aria-label={sidebarCollapsed ? "展开侧边栏" : "收起侧边栏"}
            >
              <svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
                <path d="M740.395 188c88.365 0 160 71.635 160 160v329.347c-0.001 88.365-71.635 159.999-160 160H280c-88.366 0-160-71.635-160-160V348c0-88.365 71.634-160 160-160h460.395zM280 252c-53.019 0-96 42.981-96 96v329.347c0 53.019 42.981 96 96 96h134.051V252H280z m198.051 521.347h262.344c53.019-0.001 95.999-42.981 96-96V348c0-53.019-42.981-96-96-96H478.051v521.347zM337.367 456.266c17.673 0 32 14.326 32 32 0 17.673-14.327 32-32 32H264.33c-17.673-0.001-32-14.327-32-32 0-17.673 14.327-32 32-32h73.037z m0-122.86c17.673 0 32 14.327 32 32 0 17.673-14.327 32-32 32H264.33c-17.673 0-32-14.327-32-32 0-17.673 14.327-32 32-32h73.037z" />
              </svg>
            </button>
            <h1>OpenJax</h1>
          </div>
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
          onNewChat={() => void newChat()}
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
