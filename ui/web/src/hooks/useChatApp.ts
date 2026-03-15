import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GatewayClient } from "../lib/gatewayClient";
import { applyStreamEvents } from "../lib/eventReducer";
import { recordDeltaReceived } from "../lib/streamPerf";
import { streamRenderStore } from "../lib/streamRenderStore";
import { humanizeError, isAuthenticationError } from "../lib/errors";
import { loadAuth, loadSessions, loadSettings, saveAuth, saveSessions, saveSettings } from "../lib/storage";
import type { ChatMessage, ChatSession, ChatState, MessageRole, PendingApproval } from "../types/chat";
import type {
  AppSettings,
  AuthSessionItem,
  GatewaySessionMessage,
  GatewaySessionSummary,
  GatewayConnection,
  GatewayError,
  LlmProvider,
  StreamEvent
} from "../types/gateway";

const MAX_RECONNECT_RETRY = 6;
const STREAM_DEBUG_ENABLED = resolveWebStreamDebugEnabled();

function createLocalSession(sessionId: string): ChatSession {
  return {
    id: sessionId,
    title: "新聊天",
    createdAt: new Date().toISOString(),
    connection: "idle",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: []
  };
}

function isSequenceResetEvent(event: StreamEvent): boolean {
  if (event.event_seq === 1) {
    return true;
  }
  return event.turn_seq === 1 && event.type === "response_started";
}

export function useChatApp() {
  const initialSessions = loadSessions();
  const [state, setState] = useState<ChatState>(() => ({
    settings: loadSettings(),
    auth: loadAuth(),
    sessions: initialSessions,
    activeSessionId: initialSessions[0]?.id ?? null,
    globalError: null,
    infoToast: null,
    loading: false
  }));

  const reconnectAbortRef = useRef<AbortController | null>(null);
  const pollingAbortRef = useRef<AbortController | null>(null);
  const sessionsRef = useRef(state.sessions);
  const refreshPromiseRef = useRef<Promise<boolean> | null>(null);
  const sseRunTokenRef = useRef<Record<string, number>>({});
  const nextSseRunTokenRef = useRef(1);
  const lastEventSeqRef = useRef<Record<string, number>>(
    Object.fromEntries(initialSessions.map((session) => [session.id, session.lastEventSeq]))
  );

  useEffect(() => {
    sessionsRef.current = state.sessions;
    const nextLastSeq = { ...lastEventSeqRef.current };
    for (const session of state.sessions) {
      const prev = nextLastSeq[session.id] ?? 0;
      nextLastSeq[session.id] = Math.max(prev, session.lastEventSeq);
    }
    lastEventSeqRef.current = nextLastSeq;
  }, [state.sessions]);

  const applyIncomingEvent = useCallback((sessionId: string, event: StreamEvent) => {
    const seenSeq = lastEventSeqRef.current[sessionId] ?? 0;
    const seqReset = isSequenceResetEvent(event);
    if (!seqReset && event.event_seq <= seenSeq) {
      if (STREAM_DEBUG_ENABLED) {
        console.debug("[stream_debug][use_chat_app][drop_global_seq]", {
          sessionId,
          eventType: event.type,
          eventSeq: event.event_seq,
          turnId: event.turn_id,
          seenSeq
        });
      }
      return;
    }
    if (seqReset) {
      lastEventSeqRef.current[sessionId] = 0;
      streamRenderStore.clear(sessionId);
    }
    lastEventSeqRef.current[sessionId] = Math.max(lastEventSeqRef.current[sessionId] ?? 0, event.event_seq);

    if (event.type === "response_text_delta") {
      recordDeltaReceived(sessionId);
      streamRenderStore.append(sessionId, event.turn_id, String(event.payload.content_delta ?? ""), event.event_seq);
      setState((prev) => {
        let changed = false;
        const sessions = prev.sessions.map((session) => {
          if (session.id !== sessionId) {
            return session;
          }
          const next = closeOpenReasoningBlockInSession(session, event.turn_id);
          changed = changed || next !== session;
          return next;
        });
        return changed ? { ...prev, sessions } : prev;
      });
      if (STREAM_DEBUG_ENABLED) {
        console.debug("[stream_debug][use_chat_app][delta_store_append]", {
          sessionId,
          eventSeq: event.event_seq,
          turnId: event.turn_id,
          delta: String(event.payload.content_delta ?? "")
        });
      }
      return;
    }

    if (event.type === "response_started") {
      let startedMessageId: string | undefined;
      let startedContent = "";
      setState((prev) => {
        let changed = false;
        const sessions = prev.sessions.map((session) => {
          if (session.id !== sessionId) {
            return session;
          }
          const next = applyResponseStartedSession(session, event);
          startedMessageId = next.messageId;
          startedContent = next.content;
          changed = changed || next.session !== session;
          return { ...next.session, connection: "active" as const };
        });
        return changed ? { ...prev, sessions } : prev;
      });
      streamRenderStore.start(sessionId, event.turn_id, startedMessageId, event.event_seq, startedContent);
      return;
    }

    if (event.type === "response_completed" || event.type === "assistant_message") {
      const payloadContent = String(event.payload.content ?? "");
      streamRenderStore.complete(sessionId, event.turn_id, payloadContent, event.event_seq);
      const snapshot = streamRenderStore.getSnapshot(sessionId, event.turn_id);
      const finalizedContent = payloadContent.length > 0 ? payloadContent : snapshot.content;
      setState((prev) => {
        let changed = false;
        const sessions = prev.sessions.map((session) => {
          if (session.id !== sessionId) {
            return session;
          }
          const withReasoningClosed =
            event.type === "response_completed"
              ? closeOpenReasoningBlockInSession(session, event.turn_id)
              : session;
          const next = applyResponseCompletedSession(withReasoningClosed, event, finalizedContent);
          changed = changed || next !== session;
          return { ...next, connection: "active" as const };
        });
        return changed ? { ...prev, sessions } : prev;
      });
      return;
    }

    if (event.type === "response_error") {
      streamRenderStore.fail(sessionId, event.turn_id, event.event_seq);
    }
    if (event.type === "turn_completed") {
      streamRenderStore.clear(sessionId, event.turn_id);
    }

    setState((prev) => {
      let changed = false;
      const sessions = prev.sessions.map((session) => {
        if (session.id !== sessionId) {
          return session;
        }
        const next = applyStreamEvents(session, [event]);
        changed = changed || next !== session;
        return { ...next, connection: "active" as const };
      });
      return changed ? { ...prev, sessions } : prev;
    });
  }, []);

  useEffect(() => {
    saveSessions(state.sessions);
  }, [state.sessions]);

  const connection = useMemo<GatewayConnection>(
    () => ({
      baseUrl: state.settings.baseUrl,
      accessToken: state.auth.accessToken
    }),
    [state.auth.accessToken, state.settings.baseUrl]
  );
  const client = useMemo(() => new GatewayClient(connection), [connection]);

  const activeSession = useMemo(
    () => state.sessions.find((session) => session.id === state.activeSessionId) ?? null,
    [state.activeSessionId, state.sessions]
  );

  const clearAuthState = useCallback((message: string) => {
    reconnectAbortRef.current?.abort();
    pollingAbortRef.current?.abort();
    for (const session of sessionsRef.current) {
      streamRenderStore.clear(session.id);
    }
    saveAuth({ authenticated: false, accessToken: "", sessionId: null, scope: null });
    setState((prev) => ({
      ...prev,
      auth: { authenticated: false, accessToken: "", sessionId: null, scope: null },
      sessions: [],
      activeSessionId: null,
      globalError: message
    }));
  }, []);

  const refreshAccessToken = useCallback(async (): Promise<boolean> => {
    if (refreshPromiseRef.current) {
      return refreshPromiseRef.current;
    }

    const refreshing = (async () => {
      try {
        const refreshed = await client.refresh();
        saveAuth({
          authenticated: true,
          accessToken: "",
          sessionId: refreshed.session_id,
          scope: refreshed.scope
        });
        setState((prev) => ({
          ...prev,
          auth: {
            authenticated: true,
            accessToken: refreshed.access_token,
            sessionId: refreshed.session_id,
            scope: refreshed.scope
          }
        }));
        return true;
      } catch {
        clearAuthState("登录态已失效，请重新登录。");
        return false;
      } finally {
        refreshPromiseRef.current = null;
      }
    })();

    refreshPromiseRef.current = refreshing;
    return refreshing;
  }, [clearAuthState, client]);

  const withAuthRetry = useCallback(
    async <T>(action: () => Promise<T>): Promise<T> => {
      try {
        return await action();
      } catch (error) {
        if (!isAuthenticationError(error)) {
          throw error;
        }

        const refreshed = await refreshAccessToken();
        if (!refreshed) {
          throw error;
        }
        return action();
      }
    },
    [refreshAccessToken]
  );

  useEffect(() => {
    if (!state.auth.authenticated || state.auth.accessToken) {
      return;
    }
    void refreshAccessToken();
  }, [refreshAccessToken, state.auth.accessToken, state.auth.authenticated]);

  const updateSession = useCallback((sessionId: string, updater: (session: ChatSession) => ChatSession) => {
    setState((prev) => ({
      ...prev,
      sessions: prev.sessions.map((session) => (session.id === sessionId ? updater(session) : session))
    }));
  }, []);

  const ensureSession = useCallback(async (): Promise<string> => {
    if (state.activeSessionId) {
      return state.activeSessionId;
    }

    setState((prev) => ({ ...prev, loading: true, globalError: null }));
    try {
      const created = await withAuthRetry(() => client.startSession());
      const session = createLocalSession(created.session_id);
      setState((prev) => ({
        ...prev,
        sessions: [session, ...prev.sessions],
        activeSessionId: session.id,
        loading: false
      }));
      return session.id;
    } catch (error) {
      if (isAuthenticationError(error)) {
        clearAuthState("认证失效，请重新登录。");
      } else {
        setState((prev) => ({ ...prev, loading: false, globalError: humanizeError(error) }));
      }
      throw error;
    }
  }, [clearAuthState, client, state.activeSessionId, withAuthRetry]);

  const startSseLoop = useCallback(
    (sessionId: string) => {
      reconnectAbortRef.current?.abort();
      const abort = new AbortController();
      reconnectAbortRef.current = abort;
      const runToken = nextSseRunTokenRef.current++;
      sseRunTokenRef.current[sessionId] = runToken;

      let retry = 0;

      const run = async () => {
        while (!abort.signal.aborted) {
          if (sseRunTokenRef.current[sessionId] !== runToken) {
            return;
          }
          try {
            updateSession(sessionId, (session) => ({ ...session, connection: "connecting" }));
            const lastSeq =
              lastEventSeqRef.current[sessionId] ??
              sessionsRef.current.find((session) => session.id === sessionId)?.lastEventSeq ??
              0;
            await client.streamEvents({
              sessionId,
              afterEventSeq: lastSeq,
              signal: abort.signal,
              onEvent: (event) => {
                if (sseRunTokenRef.current[sessionId] !== runToken) {
                  return;
                }
                applyIncomingEvent(sessionId, event);
              },
              onError: (error) => {
                setState((prev) => ({ ...prev, globalError: `SSE 事件解析失败: ${error.message}` }));
              }
            });
            retry = 0;
          } catch (error) {
            if (abort.signal.aborted) {
              return;
            }
            if (sseRunTokenRef.current[sessionId] !== runToken) {
              return;
            }
            if (isAuthenticationError(error)) {
              const refreshed = await refreshAccessToken();
              if (!refreshed) {
                updateSession(sessionId, (session) => ({ ...session, connection: "closed" }));
                return;
              }
              continue;
            }
            if (isSessionNotFoundError(error)) {
              setState((prev) => {
                const removedIndex = prev.sessions.findIndex((session) => session.id === sessionId);
                if (removedIndex < 0) {
                  return prev;
                }
                const sessions = prev.sessions.filter((session) => session.id !== sessionId);
                const nextActive = sessions[removedIndex] ?? sessions[removedIndex - 1] ?? null;
                return {
                  ...prev,
                  sessions,
                  activeSessionId: nextActive?.id ?? null,
                  globalError: null,
                  infoToast: "检测到失效会话，已自动移除。"
                };
              });
              return;
            }
            retry += 1;
            updateSession(sessionId, (session) => ({ ...session, connection: "connecting" }));
            setState((prev) => ({
              ...prev,
              globalError: `连接中断，正在重连 (${Math.min(retry, MAX_RECONNECT_RETRY)}/${MAX_RECONNECT_RETRY})`
            }));
            if (retry > MAX_RECONNECT_RETRY) {
              setState((prev) => ({
                ...prev,
                globalError: "SSE 重连失败，请切换为 Polling 或稍后重试。"
              }));
              return;
            }
            await new Promise((resolve) => setTimeout(resolve, 300 * 2 ** retry));
          }
        }
      };

      void run();
    },
    [applyIncomingEvent, client, refreshAccessToken, updateSession]
  );

  useEffect(() => {
    if (!state.auth.authenticated || !state.auth.accessToken || !activeSession || state.settings.outputMode !== "sse") {
      reconnectAbortRef.current?.abort();
      return;
    }
    startSseLoop(activeSession.id);
    return () => {
      reconnectAbortRef.current?.abort();
    };
  }, [
    activeSession?.id,
    startSseLoop,
    state.auth.accessToken,
    state.auth.authenticated,
    state.settings.outputMode
  ]);

  const hydrateSessionsFromGateway = useCallback(async (apiClient: GatewayClient): Promise<ChatSession[]> => {
    const sessionsResponse = await apiClient.listChatSessions();
    const hydrated: ChatSession[] = [];
    for (const remoteSession of sessionsResponse.sessions) {
      try {
        const messagesResponse = await apiClient.listSessionMessages(remoteSession.session_id);
        hydrated.push(buildChatSessionFromGateway(remoteSession, messagesResponse.messages));
      } catch {
        continue;
      }
    }
    const deduped = new Map<string, ChatSession>();
    for (const session of hydrated) {
      if (!deduped.has(session.id)) {
        deduped.set(session.id, session);
      }
    }
    return Array.from(deduped.values());
  }, []);

  const authenticate = useCallback(
    async (baseUrlInput: string, ownerKeyInput: string) => {
      const baseUrl = baseUrlInput.trim();
      const ownerKey = ownerKeyInput.trim();
      if (!baseUrl || !ownerKey) {
        setState((prev) => ({ ...prev, globalError: "请填写 Gateway 地址和 Owner Key。" }));
        return false;
      }

      const tempClient = new GatewayClient({ baseUrl, accessToken: "" });
      try {
        const result = await tempClient.login(ownerKey);
        const authedClient = new GatewayClient({
          baseUrl,
          accessToken: result.access_token
        });
        let sessions: ChatSession[] = [];
        let toast = "登录成功";
        try {
          sessions = await hydrateSessionsFromGateway(authedClient);
          toast = sessions.length > 0 ? `登录成功，已同步 ${sessions.length} 个历史会话` : "登录成功，暂无历史会话";
        } catch {
          sessions = loadSessions();
          toast = "历史同步失败，已使用本地缓存";
        }
        for (const session of sessionsRef.current) {
          streamRenderStore.clear(session.id);
        }
        lastEventSeqRef.current = Object.fromEntries(
          sessions.map((session) => [session.id, session.lastEventSeq])
        );

        const settings = {
          ...state.settings,
          baseUrl
        };
        saveSettings(settings);
        saveAuth({
          authenticated: true,
          accessToken: "",
          sessionId: result.session_id,
          scope: result.scope
        });
        setState((prev) => ({
          ...prev,
          settings,
          auth: {
            authenticated: true,
            accessToken: result.access_token,
            sessionId: result.session_id,
            scope: result.scope
          },
          sessions,
          activeSessionId: sessions[0]?.id ?? null,
          globalError: null,
          infoToast: toast
        }));
        return true;
      } catch (error) {
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
        return false;
      }
    },
    [hydrateSessionsFromGateway, state.settings]
  );

  const logout = useCallback(async () => {
    reconnectAbortRef.current?.abort();
    pollingAbortRef.current?.abort();
    for (const session of sessionsRef.current) {
      streamRenderStore.clear(session.id);
    }
    if (state.auth.sessionId) {
      try {
        await client.logout(state.auth.sessionId);
      } catch {
        // Best effort logout.
      }
    }
    saveAuth({ authenticated: false, accessToken: "", sessionId: null, scope: null });
    setState((prev) => ({
      ...prev,
      auth: { authenticated: false, accessToken: "", sessionId: null, scope: null },
      sessions: [],
      activeSessionId: null,
      globalError: null,
      infoToast: null
    }));
  }, [client, state.auth.sessionId]);

  const sendMessage = useCallback(
    async (content: string) => {
      const message = content.trim();
      if (!message) {
        return;
      }

      const sessionId = await ensureSession();
      const userMessage: ChatMessage = {
        id: crypto.randomUUID(),
        kind: "text",
        role: "user",
        content: message,
        timestamp: new Date().toISOString()
      };

      updateSession(sessionId, (session) => ({
        ...session,
        turnPhase: "submitting",
        title: session.messages.length === 0 ? summarizeTitle(message) : session.title,
        messages: [...session.messages, userMessage]
      }));

      try {
        const submitted = await withAuthRetry(() => client.submitTurn(sessionId, message));
        if (state.settings.outputMode === "polling") {
          pollingAbortRef.current?.abort();
          const pollAbort = new AbortController();
          pollingAbortRef.current = pollAbort;

          updateSession(sessionId, (session) => ({ ...session, connection: "active", turnPhase: "streaming" }));
          const result = await withAuthRetry(() =>
            client.pollTurnUntilDone(sessionId, submitted.turn_id, pollAbort.signal)
          );

          if (result.status === "completed") {
            updateSession(sessionId, (session) => ({
              ...session,
              turnPhase: "completed",
              messages: [
                ...session.messages,
                {
                  id: crypto.randomUUID(),
                  kind: "text",
                  role: "assistant",
                  content: result.assistant_message ?? "",
                  timestamp: result.timestamp,
                  turnId: result.turn_id
                }
              ]
            }));
          } else {
            updateSession(sessionId, (session) => ({
              ...session,
              turnPhase: "failed",
              messages: [
                ...session.messages,
                {
                  id: crypto.randomUUID(),
                  kind: "text",
                  role: "error",
                  content: result.error?.message ?? "回合失败",
                  timestamp: result.timestamp,
                  turnId: result.turn_id
                }
              ]
            }));
          }
        }
      } catch (error) {
        updateSession(sessionId, (session) => ({ ...session, turnPhase: "failed" }));
        if (isAuthenticationError(error)) {
          clearAuthState("登录态已失效，请重新登录。");
          return;
        }
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      }
    },
    [
      clearAuthState,
      client,
      ensureSession,
      state.settings.outputMode,
      updateSession,
      withAuthRetry
    ]
  );

  const newChat = useCallback(async () => {
    try {
      const created = await withAuthRetry(() => client.startSession());
      const session = createLocalSession(created.session_id);
      setState((prev) => ({
        ...prev,
        sessions: [session, ...prev.sessions],
        activeSessionId: session.id,
        globalError: null
      }));
    } catch (error) {
      if (isAuthenticationError(error)) {
        clearAuthState("登录态已失效，请重新登录。");
        return;
      }
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [clearAuthState, client, withAuthRetry]);

  const switchSession = useCallback((sessionId: string) => {
    setState((prev) => ({ ...prev, activeSessionId: sessionId, globalError: null }));
  }, []);

  const deleteSession = useCallback(
    async (sessionId: string) => {
      const shouldDelete = window.confirm("确认删除该会话？此操作不可恢复。");
      if (!shouldDelete) {
        return;
      }

      if (state.activeSessionId === sessionId) {
        reconnectAbortRef.current?.abort();
        pollingAbortRef.current?.abort();
      }

      try {
        await withAuthRetry(() => client.shutdownSession(sessionId));
        streamRenderStore.clear(sessionId);
        setState((prev) => {
          const removedIndex = prev.sessions.findIndex((session) => session.id === sessionId);
          if (removedIndex < 0) {
            return prev;
          }

          const sessions = prev.sessions.filter((session) => session.id !== sessionId);
          const nextActive =
            prev.activeSessionId === sessionId
              ? sessions[removedIndex] ?? sessions[removedIndex - 1] ?? null
              : sessions.find((session) => session.id === prev.activeSessionId) ?? null;

          return {
            ...prev,
            sessions,
            activeSessionId: nextActive?.id ?? null,
            globalError: null,
            infoToast: "会话已删除"
          };
        });
      } catch (error) {
        if (isAuthenticationError(error)) {
          clearAuthState("登录态已失效，请重新登录。");
          return;
        }
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      }
    },
    [clearAuthState, client, state.activeSessionId, withAuthRetry]
  );

  const resolveApproval = useCallback(
    async (approval: PendingApproval, approved: boolean) => {
      const sessionId = state.activeSessionId;
      if (!sessionId) {
        return;
      }

      try {
        await withAuthRetry(() =>
          client.resolveApproval(sessionId, approval.approvalId, approved, approved ? "approved" : "rejected")
        );
        updateSession(sessionId, (session) => ({
          ...session,
          pendingApprovals: session.pendingApprovals.filter(
            (item) => item.approvalId !== approval.approvalId
          )
        }));
      } catch (error) {
        if (isAuthenticationError(error)) {
          clearAuthState("登录态已失效，请重新登录。");
          return;
        }
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      }
    },
    [clearAuthState, client, state.activeSessionId, updateSession, withAuthRetry]
  );

  const clearConversation = useCallback(async () => {
    if (!state.activeSessionId) {
      return;
    }
    try {
      await withAuthRetry(() => client.clearSession(state.activeSessionId!));
      streamRenderStore.clear(state.activeSessionId);
      updateSession(state.activeSessionId, (session) => ({
        ...session,
        messages: [],
        pendingApprovals: [],
        turnPhase: "draft"
      }));
      setState((prev) => ({ ...prev, infoToast: "会话已清空" }));
    } catch (error) {
      if (isAuthenticationError(error)) {
        clearAuthState("登录态已失效，请重新登录。");
        return;
      }
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [clearAuthState, client, state.activeSessionId, updateSession, withAuthRetry]);

  const compactConversation = useCallback(async () => {
    if (!state.activeSessionId) {
      return;
    }
    try {
      await withAuthRetry(() => client.compactSession(state.activeSessionId!));
      setState((prev) => ({ ...prev, infoToast: "会话已压缩" }));
    } catch (error) {
      if (isAuthenticationError(error)) {
        clearAuthState("登录态已失效，请重新登录。");
        return;
      }
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [clearAuthState, client, state.activeSessionId, withAuthRetry]);

  const updateSettings = useCallback((next: AppSettings) => {
    const normalizedSettings: AppSettings = {
      ...next,
      baseUrl: next.baseUrl.trim(),
      selectedProviderId: next.selectedProviderId?.trim() ? next.selectedProviderId.trim() : null,
      selectedModelName: next.selectedModelName?.trim() ? next.selectedModelName.trim() : null
    };
    saveSettings(normalizedSettings);
    setState((prev) => ({
      ...prev,
      settings: normalizedSettings,
      globalError: null,
      infoToast: "设置已保存"
    }));
  }, []);

  const testConnection = useCallback(async (nextSettings: AppSettings) => {
    const tempClient = new GatewayClient({
      baseUrl: nextSettings.baseUrl,
      accessToken: state.auth.accessToken
    });
    try {
      const result = await tempClient.healthCheck();
      setState((prev) => ({ ...prev, infoToast: `连接成功: ${result.status}` }));
      return true;
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      return false;
    }
  }, [state.auth.accessToken]);

  const listAuthSessions = useCallback(async (): Promise<AuthSessionItem[]> => {
    const data = await withAuthRetry(() => client.listSessions());
    return data.sessions;
  }, [client, withAuthRetry]);

  const revokeAuthSession = useCallback(async (sessionId: string) => {
    await withAuthRetry(() => client.revoke({ sessionId }));
  }, [client, withAuthRetry]);

  const revokeAllAuthSessions = useCallback(async () => {
    await withAuthRetry(() => client.revoke({ revokeAll: true }));
    clearAuthState("当前设备会话已失效，请重新登录。");
  }, [clearAuthState, client, withAuthRetry]);

  const listProviders = useCallback(async (): Promise<LlmProvider[]> => {
    const data = await withAuthRetry(() => client.listProviders());
    return data.providers;
  }, [client, withAuthRetry]);

  const createProvider = useCallback(
    async (payload: {
      providerName: string;
      baseUrl: string;
      modelName: string;
      apiKey: string;
    }): Promise<LlmProvider> => {
      const data = await withAuthRetry(() => client.createProvider(payload));
      return data.provider;
    },
    [client, withAuthRetry]
  );

  const updateProvider = useCallback(
    async (
      providerId: string,
      payload: {
        providerName: string;
        baseUrl: string;
        modelName: string;
        apiKey?: string;
      }
    ): Promise<LlmProvider> => {
      const data = await withAuthRetry(() => client.updateProvider(providerId, payload));
      return data.provider;
    },
    [client, withAuthRetry]
  );

  const deleteProvider = useCallback(
    async (providerId: string): Promise<void> => {
      await withAuthRetry(() => client.deleteProvider(providerId));
    },
    [client, withAuthRetry]
  );

  const getActiveProvider = useCallback(async (): Promise<LlmProvider | null> => {
    const [providersData, activeData] = await Promise.all([
      withAuthRetry(() => client.listProviders()),
      withAuthRetry(() => client.getActiveProvider())
    ]);
    const providerId = activeData.active_provider?.provider_id;
    if (!providerId) {
      return null;
    }
    return providersData.providers.find((item) => item.provider_id === providerId) ?? null;
  }, [client, withAuthRetry]);

  const setActiveProvider = useCallback(
    async (providerId: string): Promise<LlmProvider> => {
      const [providersData, activeData] = await Promise.all([
        withAuthRetry(() => client.listProviders()),
        withAuthRetry(() => client.setActiveProvider(providerId))
      ]);
      const selectedId = activeData.active_provider?.provider_id ?? providerId;
      const selected =
        providersData.providers.find((item) => item.provider_id === selectedId) ??
        providersData.providers.find((item) => item.provider_id === providerId);
      if (!selected) {
        throw new Error("已设置 active provider，但未在 Provider 列表中找到该项。");
      }
      setState((prev) => {
        const settings: AppSettings = {
          ...prev.settings,
          selectedProviderId: selected.provider_id,
          selectedModelName: selected.model_name
        };
        saveSettings(settings);
        return {
          ...prev,
          settings,
          globalError: null
        };
      });
      return selected;
    },
    [client, withAuthRetry]
  );

  const dismissGlobalError = useCallback(() => {
    setState((prev) => ({ ...prev, globalError: null }));
  }, []);

  const dismissToast = useCallback(() => {
    setState((prev) => ({ ...prev, infoToast: null }));
  }, []);

  return {
    state,
    activeSession,
    isAuthenticated: state.auth.authenticated,
    authenticate,
    logout,
    newChat,
    switchSession,
    deleteSession,
    sendMessage,
    resolveApproval,
    clearConversation,
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
    getActiveProvider,
    setActiveProvider,
    dismissGlobalError,
    dismissToast
  };
}

function summarizeTitle(input: string): string {
  const plain = input.replace(/\s+/g, " ").trim();
  return plain.length > 24 ? `${plain.slice(0, 24)}...` : plain;
}

export function mapGatewayRoleToMessageRole(role: string): MessageRole {
  if (role === "user" || role === "assistant" || role === "tool" || role === "error" || role === "system") {
    return role;
  }
  return "system";
}

export function buildChatSessionFromGateway(
  remoteSession: GatewaySessionSummary,
  remoteMessages: GatewaySessionMessage[]
): ChatSession {
  const orderedMessages = [...remoteMessages].sort((left, right) => left.sequence - right.sequence);
  const messages: ChatMessage[] = orderedMessages.map((item) => ({
    id: item.message_id,
    kind: "text",
    role: mapGatewayRoleToMessageRole(item.role),
    content: item.content,
    timestamp: item.created_at,
    turnId: item.turn_id
  }));
  const firstUserMessage = messages.find((message) => message.role === "user");
  const title = remoteSession.title?.trim() || summarizeTitle(firstUserMessage?.content ?? "新聊天");
  return {
    id: remoteSession.session_id,
    title,
    createdAt: remoteSession.created_at,
    connection: "idle",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages,
    pendingApprovals: []
  };
}

function applyResponseStartedSession(
  session: ChatSession,
  event: StreamEvent
): { session: ChatSession; messageId?: string; content: string } {
  const turnId = event.turn_id;
  if (!turnId) {
    return { session, content: "" };
  }

  const existingIndex = findAssistantMessageIndex(session.messages, turnId);
  const messages = [...session.messages];
  let messageId: string;
  let content = "";
  if (existingIndex >= 0) {
    const existing = messages[existingIndex];
    messageId = existing.id;
    content = existing.content;
    messages[existingIndex] = {
      ...existing,
      isDraft: true,
      timestamp: event.timestamp
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: "",
      timestamp: event.timestamp,
      turnId,
      isDraft: true
    });
  }

  return {
    session: {
      ...session,
      turnPhase: "streaming",
      lastEventSeq: Math.max(session.lastEventSeq, event.event_seq),
      messages,
      streaming: {
        turnId,
        assistantMessageId: messageId,
        content,
        lastEventSeq: event.event_seq,
        active: true
      }
    },
    messageId,
    content
  };
}

function applyResponseCompletedSession(session: ChatSession, event: StreamEvent, finalizedContent: string): ChatSession {
  const turnId = event.turn_id;
  if (!turnId) {
    return session;
  }

  let nextContent = finalizedContent;
  const messages = [...session.messages];
  const index = findAssistantMessageIndex(messages, turnId);
  let messageId: string;
  if (index >= 0) {
    const message = messages[index];
    if (event.type === "assistant_message" && message.content.length > nextContent.length) {
      // Guard against late, shorter assistant_message payload overriding a fuller finalized body.
      nextContent = message.content;
    }
    messageId = message.id;
    messages[index] = {
      ...message,
      content: nextContent,
      isDraft: false,
      timestamp: event.timestamp
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: nextContent,
      timestamp: event.timestamp,
      turnId,
      isDraft: false
    });
  }

  return {
    ...session,
    turnPhase: "completed",
    lastEventSeq: Math.max(session.lastEventSeq, event.event_seq),
    messages,
    streaming: {
      turnId,
      assistantMessageId: messageId,
      content: nextContent,
      lastEventSeq: event.event_seq,
      active: false
    }
  };
}

function findAssistantMessageIndex(messages: ChatMessage[], turnId: string): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.turnId !== turnId || message.role !== "assistant" || message.kind !== "text") {
      continue;
    }
    return i;
  }
  return -1;
}

function buildAssistantDraftId(turnId: string): string {
  return `assistant_draft_${turnId}`;
}

function closeOpenReasoningBlockInSession(session: ChatSession, turnId?: string): ChatSession {
  if (!turnId) {
    return session;
  }
  const idx = findAssistantMessageIndex(session.messages, turnId);
  if (idx < 0) {
    return session;
  }
  const message = session.messages[idx];
  const blocks = message.reasoningBlocks;
  if (!blocks || blocks.length === 0) {
    return session;
  }
  let openIdx = -1;
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    if (!blocks[i].closed) {
      openIdx = i;
      break;
    }
  }
  if (openIdx < 0) {
    return session;
  }
  const nextBlocks = [...blocks];
  nextBlocks[openIdx] = {
    ...nextBlocks[openIdx],
    closed: true
  };
  const messages = [...session.messages];
  messages[idx] = {
    ...message,
    reasoningBlocks: nextBlocks
  };
  return {
    ...session,
    messages
  };
}

function isSessionNotFoundError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const gateway = error as Partial<GatewayError>;
  return gateway.code === "NOT_FOUND" || gateway.status === 404;
}

function resolveWebStreamDebugEnabled(): boolean {
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
          VITE_OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
        })
      : {};
  const raw = String(
    globals.OPENJAX_WEB_STREAM_DEBUG ??
      globals.VITE_OPENJAX_WEB_STREAM_DEBUG ??
      "0"
  )
    .trim()
    .toLowerCase();
  return !(raw === "0" || raw === "off" || raw === "false" || raw === "disabled");
}
