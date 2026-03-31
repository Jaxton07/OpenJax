import { useEffect, useMemo, useState } from "react";
import Composer from "./components/composer";
import LoginPage from "./components/LoginPage";
import MessageList from "./components/MessageList";
import SettingsModal from "./components/SettingsModal";
import Sidebar from "./components/Sidebar";
import ThemeToggle from "./components/ThemeToggle";
import ToastBanner from "./components/ToastBanner";
import { useChatApp } from "./hooks/useChatApp";
import { SettingsIcon, SidebarToggleIcon } from "./pic/icon";

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
    loadMoreSessions,
    sidebarHasMore,
    sidebarLoadingMore,
    switchSession,
    deleteSession,
    sendMessage,
    resolveApproval,
    updateSettings,
    testConnection,
    listAuthSessions,
    revokeAuthSession,
    revokeAllAuthSessions,
    listProviders,
    createProvider,
    updateProvider,
    deleteProvider,
    getActiveProvider,
    setActiveProvider,
    fetchCatalog,
    dismissGlobalError,
    dismissToast,
    sendPolicyLevel,
    isStreaming,
    isBusyTurn,
    abortTurn,
    clearConversation,
    notifyBusyTurnBlockedSend
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
        onLoadMoreSessions={() => void loadMoreSessions()}
        hasMoreSessions={sidebarHasMore}
        loadingMoreSessions={sidebarLoadingMore}
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
          <div className="chat-header-right">
            <ThemeToggle />
            <button
              className="header-settings-btn"
              onClick={() => setSettingsOpen(true)}
              title="设置"
              aria-label="打开设置"
            >
              <SettingsIcon aria-hidden="true" />
            </button>
            <div className="chat-status">{activeSession?.connection ?? "idle"}</div>
          </div>
        </header>

        <div className="chat-banners">
          {state.globalError ? (
            <ToastBanner
              message={state.globalError}
              variant="error"
              duration={6000}
              onDismiss={dismissGlobalError}
            />
          ) : null}
          {state.infoToast ? (
            <ToastBanner
              message={state.infoToast}
              variant="info"
              duration={4000}
              onDismiss={dismissToast}
            />
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
          baseUrl={state.settings.baseUrl}
          accessToken={state.auth.accessToken}
          sessionId={activeSession?.id}
          contextUsage={activeSession?.contextUsage ?? null}
          onSend={sendMessage}
          onNewChat={() => void newChat()}
          onClear={() => void clearConversation()}
          policyLevel={activeSession?.policyLevel ?? "ask"}
          onPolicyLevelChange={(level) => void sendPolicyLevel(activeSession!.id, level)}
          isBusyTurn={isBusyTurn}
          isStreaming={isStreaming}
          onBlockedSendAttempt={notifyBusyTurnBlockedSend}
          onStop={() => void abortTurn()}
        />
      </main>

      <SettingsModal
        open={settingsOpen}
        initialSettings={state.settings}
        onClose={() => setSettingsOpen(false)}
        onSave={updateSettings}
        onTest={testConnection}
        onListProviders={listProviders}
        onCreateProvider={createProvider}
        onUpdateProvider={updateProvider}
        onDeleteProvider={deleteProvider}
        onGetActiveProvider={getActiveProvider}
        onSetActiveProvider={setActiveProvider}
        onFetchCatalog={fetchCatalog}
      />
    </div>
  );
}
