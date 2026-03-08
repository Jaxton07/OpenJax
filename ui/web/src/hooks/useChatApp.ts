import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GatewayClient } from "../lib/gatewayClient";
import { applyStreamEvent } from "../lib/eventReducer";
import { humanizeError } from "../lib/errors";
import { loadSettings, loadSessions, saveSessions, saveSettings } from "../lib/storage";
import type { ChatMessage, ChatSession, ChatState, PendingApproval } from "../types/chat";
import type { AppSettings } from "../types/gateway";

const MAX_RECONNECT_RETRY = 6;

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

export function useChatApp() {
  const [state, setState] = useState<ChatState>(() => ({
    settings: loadSettings(),
    sessions: loadSessions(),
    activeSessionId: loadSessions()[0]?.id ?? null,
    globalError: null,
    infoToast: null,
    loading: false
  }));

  const reconnectAbortRef = useRef<AbortController | null>(null);
  const pollingAbortRef = useRef<AbortController | null>(null);
  const sessionsRef = useRef(state.sessions);

  useEffect(() => {
    sessionsRef.current = state.sessions;
  }, [state.sessions]);

  useEffect(() => {
    saveSessions(state.sessions);
  }, [state.sessions]);

  const client = useMemo(() => new GatewayClient(state.settings), [state.settings]);

  const activeSession = useMemo(
    () => state.sessions.find((session) => session.id === state.activeSessionId) ?? null,
    [state.activeSessionId, state.sessions]
  );

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
      const created = await client.startSession();
      const session = createLocalSession(created.session_id);
      setState((prev) => ({
        ...prev,
        sessions: [session, ...prev.sessions],
        activeSessionId: session.id,
        loading: false
      }));
      return session.id;
    } catch (error) {
      setState((prev) => ({ ...prev, loading: false, globalError: humanizeError(error) }));
      throw error;
    }
  }, [client, state.activeSessionId]);

  const startSseLoop = useCallback(
    (sessionId: string) => {
      reconnectAbortRef.current?.abort();
      const abort = new AbortController();
      reconnectAbortRef.current = abort;

      let retry = 0;

      const run = async () => {
        while (!abort.signal.aborted) {
          try {
            updateSession(sessionId, (session) => ({ ...session, connection: "connecting" }));
            const lastSeq =
              sessionsRef.current.find((session) => session.id === sessionId)?.lastEventSeq ?? 0;
            await client.streamEvents({
              sessionId,
              afterEventSeq: lastSeq,
              signal: abort.signal,
              onEvent: (event) => {
                updateSession(sessionId, (session) => {
                  const next = applyStreamEvent(session, event);
                  return { ...next, connection: "active" };
                });
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
    [client, updateSession]
  );

  useEffect(() => {
    if (!activeSession || state.settings.outputMode !== "sse") {
      reconnectAbortRef.current?.abort();
      return;
    }
    startSseLoop(activeSession.id);
    return () => reconnectAbortRef.current?.abort();
  }, [activeSession?.id, startSseLoop, state.settings.outputMode]);

  const sendMessage = useCallback(
    async (content: string) => {
      const message = content.trim();
      if (!message) {
        return;
      }

      const sessionId = await ensureSession();
      const userMessage: ChatMessage = {
        id: crypto.randomUUID(),
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
        const submitted = await client.submitTurn(sessionId, message);
        if (state.settings.outputMode === "polling") {
          pollingAbortRef.current?.abort();
          const pollAbort = new AbortController();
          pollingAbortRef.current = pollAbort;

          updateSession(sessionId, (session) => ({ ...session, connection: "active", turnPhase: "streaming" }));
          const result = await client.pollTurnUntilDone(sessionId, submitted.turn_id, pollAbort.signal);

          if (result.status === "completed") {
            updateSession(sessionId, (session) => ({
              ...session,
              turnPhase: "completed",
              messages: [
                ...session.messages,
                {
                  id: crypto.randomUUID(),
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
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      }
    },
    [client, ensureSession, state.settings.outputMode, updateSession]
  );

  const newChat = useCallback(async () => {
    try {
      const created = await client.startSession();
      const session = createLocalSession(created.session_id);
      setState((prev) => ({
        ...prev,
        sessions: [session, ...prev.sessions],
        activeSessionId: session.id,
        globalError: null
      }));
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [client]);

  const switchSession = useCallback((sessionId: string) => {
    setState((prev) => ({ ...prev, activeSessionId: sessionId, globalError: null }));
  }, []);

  const resolveApproval = useCallback(
    async (approval: PendingApproval, approved: boolean) => {
      const sessionId = state.activeSessionId;
      if (!sessionId) {
        return;
      }

      try {
        await client.resolveApproval(sessionId, approval.approvalId, approved, approved ? "approved" : "rejected");
        updateSession(sessionId, (session) => ({
          ...session,
          pendingApprovals: session.pendingApprovals.filter(
            (item) => item.approvalId !== approval.approvalId
          )
        }));
      } catch (error) {
        setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      }
    },
    [client, state.activeSessionId, updateSession]
  );

  const clearConversation = useCallback(async () => {
    if (!state.activeSessionId) {
      return;
    }
    try {
      await client.clearSession(state.activeSessionId);
      updateSession(state.activeSessionId, (session) => ({
        ...session,
        messages: [],
        pendingApprovals: [],
        turnPhase: "draft"
      }));
      setState((prev) => ({ ...prev, infoToast: "会话已清空" }));
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [client, state.activeSessionId, updateSession]);

  const compactConversation = useCallback(async () => {
    if (!state.activeSessionId) {
      return;
    }
    try {
      await client.compactSession(state.activeSessionId);
      setState((prev) => ({ ...prev, infoToast: "会话已压缩" }));
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [client, state.activeSessionId]);

  const updateSettings = useCallback((next: AppSettings) => {
    const normalized: AppSettings = {
      ...next,
      apiKey: next.apiKey.trim(),
      baseUrl: next.baseUrl.trim()
    };
    saveSettings(normalized);
    setState((prev) => ({
      ...prev,
      settings: normalized,
      globalError: null,
      infoToast: "设置已保存"
    }));
  }, []);

  const testConnection = useCallback(async () => {
    const tempClient = new GatewayClient(state.settings);
    try {
      const result = await tempClient.healthCheck();
      setState((prev) => ({ ...prev, infoToast: `连接成功: ${result.status}` }));
      return true;
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
      return false;
    }
  }, [state.settings]);

  const dismissGlobalError = useCallback(() => {
    setState((prev) => ({ ...prev, globalError: null }));
  }, []);

  const dismissToast = useCallback(() => {
    setState((prev) => ({ ...prev, infoToast: null }));
  }, []);

  return {
    state,
    activeSession,
    newChat,
    switchSession,
    sendMessage,
    resolveApproval,
    clearConversation,
    compactConversation,
    updateSettings,
    testConnection,
    dismissGlobalError,
    dismissToast
  };
}

function summarizeTitle(input: string): string {
  const plain = input.replace(/\s+/g, " ").trim();
  return plain.length > 24 ? `${plain.slice(0, 24)}...` : plain;
}
