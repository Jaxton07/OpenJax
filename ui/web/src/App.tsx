import { useEffect, useMemo, useState } from "react";
import Composer from "./components/Composer";
import LoginPage from "./components/LoginPage";
import MessageList from "./components/MessageList";
import SettingsModal from "./components/SettingsModal";
import Sidebar from "./components/Sidebar";
import { useChatApp } from "./hooks/useChatApp";
import { SidebarToggleIcon } from "./pic/icon";
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
    listProviders,
    createProvider,
    updateProvider,
    deleteProvider,
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
              <SidebarToggleIcon aria-hidden="true" />
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
            sessionId={activeSession?.id}
            messages={activeSession?.messages ?? []}
            pendingApprovals={activeSession?.pendingApprovals ?? []}
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
        onListProviders={listProviders}
        onCreateProvider={createProvider}
        onUpdateProvider={updateProvider}
        onDeleteProvider={deleteProvider}
      />
    </div>
  );
}
