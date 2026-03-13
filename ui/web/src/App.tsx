import { useEffect, useMemo, useState } from "react";
import Composer from "./components/Composer";
import LoginPage from "./components/LoginPage";
import MessageList from "./components/MessageList";
import SettingsModal from "./components/SettingsModal";
import Sidebar from "./components/Sidebar";
import { useChatApp } from "./hooks/useChatApp";
import type { AppSettings } from "./types/gateway";

type AppRoute = "/login" | "/chat";

function resolveRoute(pathname: string): AppRoute {
  return pathname === "/chat" ? "/chat" : "/login";
}

function navigate(to: AppRoute, replace = false): void {
  if (window.location.pathname === to) {
    return;
  }
  if (replace) {
    window.history.replaceState(null, "", to);
  } else {
    window.history.pushState(null, "", to);
  }
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export default function App() {
  const {
    state,
    activeSession,
    isAuthenticated,
    authenticate,
    logout,
    newChat,
    switchSession,
    deleteSession,
    sendMessage,
    resolveApproval,
    compactConversation,
    updateSettings,
    testConnection,
    listAuthSessions,
    revokeAuthSession,
    revokeAllAuthSessions,
    dismissGlobalError,
    dismissToast
  } = useChatApp();

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(true);
  const [route, setRoute] = useState<AppRoute>(() => resolveRoute(window.location.pathname));

  useEffect(() => {
    const onPopState = () => setRoute(resolveRoute(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  useEffect(() => {
    if (isAuthenticated && route !== "/chat") {
      navigate("/chat", true);
      return;
    }
    if (!isAuthenticated && route !== "/login") {
      navigate("/login", true);
    }
  }, [isAuthenticated, route]);

  const loginError = useMemo(
    () => (route === "/login" ? state.globalError : null),
    [route, state.globalError]
  );

  const handleLogin = async (baseUrl: string, ownerKey: string) => {
    const ok = await authenticate(baseUrl, ownerKey);
    if (ok) {
      navigate("/chat");
    }
    return ok;
  };

  if (!isAuthenticated || route === "/login") {
    return (
      <LoginPage
        initialBaseUrl={state.settings.baseUrl}
        onLogin={handleLogin}
        errorMessage={loginError}
      />
    );
  }

  const testSettingsConnection = async (settings: AppSettings) => {
    return testConnection(settings);
  };

  const manageAuthSessions = async () => {
    const sessions = await listAuthSessions();
    if (sessions.length === 0) {
      window.alert("当前没有可管理会话。");
      return;
    }
    const summary = sessions
      .map((item, index) => {
        const name = item.device_name ?? item.platform ?? "unknown";
        return `${index + 1}. ${name} (${item.status}) ${item.session_id}`;
      })
      .join("\n");
    const input = window.prompt(
      `会话管理：\n${summary}\n\n输入序号踢下线，输入 all 踢下线全部会话，留空取消。`
    );
    if (!input) {
      return;
    }
    if (input.trim().toLowerCase() === "all") {
      await revokeAllAuthSessions();
      return;
    }
    const index = Number(input.trim()) - 1;
    if (Number.isNaN(index) || index < 0 || index >= sessions.length) {
      window.alert("输入无效。");
      return;
    }
    await revokeAuthSession(sessions[index].session_id);
    window.alert("已撤销该会话。");
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
        onManageSessions={() => void manageAuthSessions()}
        onLogout={() => {
          void logout();
          navigate("/login");
        }}
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

        <div className="chat-banners">
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
        </div>

        <section className="chat-scroll-region">
          <MessageList
            messages={activeSession?.messages ?? []}
            pendingApprovals={activeSession?.pendingApprovals ?? []}
            streaming={activeSession?.streaming}
            onResolveApproval={(approval, approved) => resolveApproval(approval, approved)}
          />
        </section>

        <Composer
          disabled={state.loading}
          onSend={sendMessage}
          onNewChat={() => void newChat()}
          onCompact={() => void compactConversation()}
        />
      </main>

      <SettingsModal
        open={settingsOpen}
        initialSettings={state.settings}
        onClose={() => setSettingsOpen(false)}
        onSave={updateSettings}
        onTest={testSettingsConnection}
      />
    </div>
  );
}
