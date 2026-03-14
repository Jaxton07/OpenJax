import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GatewayClient } from "../lib/gatewayClient";
import { humanizeError, isAuthError } from "../lib/errors";
import { recordDeltaRecv } from "../lib/devPerf";
import { streamStore } from "../lib/streamStore";
import type { AssistantMessage, UserMessage } from "../types/chat";
import type { GatewayConnection, StreamEvent } from "../types/gateway";

const MAX_RETRY = 6;

interface PageState {
  baseUrl: string;
  ownerKey: string;
  accessToken: string;
  connected: boolean;
  sessionId: string | null;
  activeTurnId: string | null;
  users: UserMessage[];
  assistants: AssistantMessage[];
  globalError: string | null;
  info: string | null;
  sending: boolean;
  streaming: boolean;
  replayExceeded: boolean;
}

function initialState(): PageState {
  return {
    baseUrl: "http://127.0.0.1:8765",
    ownerKey: "",
    accessToken: "",
    connected: false,
    sessionId: null,
    activeTurnId: null,
    users: [],
    assistants: [],
    globalError: null,
    info: null,
    sending: false,
    streaming: false,
    replayExceeded: false
  };
}

export function useChatPage() {
  const [state, setState] = useState<PageState>(initialState);
  const streamAbortRef = useRef<AbortController | null>(null);
  const retryRef = useRef(0);
  const lastEventSeqRef = useRef(0);
  const completedTurnsRef = useRef(new Set<string>());
  const runTokenRef = useRef(1);

  const connection = useMemo<GatewayConnection>(() => ({
    baseUrl: state.baseUrl,
    accessToken: state.accessToken
  }), [state.accessToken, state.baseUrl]);

  const client = useMemo(() => new GatewayClient(connection), [connection]);

  const stopStream = useCallback(() => {
    streamAbortRef.current?.abort();
    streamAbortRef.current = null;
    setState((prev) => ({ ...prev, streaming: false }));
  }, []);

  const onStreamEvent = useCallback((event: StreamEvent) => {
    const seen = lastEventSeqRef.current;
    const resetBoundary = event.event_seq === 1 || (event.turn_seq === 1 && event.type === "response_started");

    if (!resetBoundary && event.event_seq <= seen) {
      return;
    }

    if (resetBoundary && seen > 0) {
      completedTurnsRef.current.clear();
      if (state.sessionId) {
        streamStore.clearSession(state.sessionId);
      }
    }

    lastEventSeqRef.current = Math.max(lastEventSeqRef.current, event.event_seq);

    if (event.type === "response_started") {
      streamStore.start(event.session_id, event.turn_id, event.event_seq);
      setState((prev) => ({
        ...prev,
        activeTurnId: event.turn_id ?? prev.activeTurnId,
        streaming: true,
        globalError: null
      }));
      return;
    }

    if (event.type === "response_text_delta") {
      recordDeltaRecv(event.session_id);
      streamStore.append(event.session_id, event.turn_id, String(event.payload.content_delta ?? ""), event.event_seq);
      return;
    }

    if (event.type === "response_error" || event.type === "error") {
      streamStore.fail(event.session_id, event.turn_id, event.event_seq);
      const code = String(event.payload.code ?? "");
      const message = String(event.payload.message ?? "流式输出失败");
      const replayExceeded = code === "REPLAY_WINDOW_EXCEEDED";
      setState((prev) => ({
        ...prev,
        globalError: replayExceeded ? "流回放窗口已超限，请重新建立会话连接。" : message,
        replayExceeded,
        activeTurnId: replayExceeded ? null : prev.activeTurnId,
        streaming: !replayExceeded
      }));
      if (replayExceeded) {
        stopStream();
      }
      return;
    }

    if (event.type === "assistant_message" || event.type === "response_completed") {
      const content = String(event.payload.content ?? "");
      streamStore.complete(event.session_id, event.turn_id, content, event.event_seq);
      if (!event.turn_id || completedTurnsRef.current.has(event.turn_id)) {
        return;
      }
      completedTurnsRef.current.add(event.turn_id);
      setState((prev) => ({
        ...prev,
        assistants: [
          ...prev.assistants,
          {
            id: crypto.randomUUID(),
            role: "assistant",
            content,
            timestamp: event.timestamp,
            turnId: event.turn_id
          }
        ],
        activeTurnId: prev.activeTurnId === event.turn_id ? null : prev.activeTurnId,
        globalError: null
      }));
      return;
    }

    if (event.type === "turn_completed" && event.turn_id) {
      setState((prev) => ({
        ...prev,
        activeTurnId: prev.activeTurnId === event.turn_id ? null : prev.activeTurnId
      }));
    }
  }, [state.sessionId, stopStream]);

  const startStreamLoop = useCallback((sessionId: string) => {
    stopStream();
    const abort = new AbortController();
    streamAbortRef.current = abort;
    retryRef.current = 0;
    const runToken = runTokenRef.current++;

    setState((prev) => ({ ...prev, streaming: true, replayExceeded: false }));

    const run = async () => {
      while (!abort.signal.aborted) {
        if (runToken !== runTokenRef.current - 1) {
          return;
        }
        try {
          await client.streamEvents({
            sessionId,
            afterEventSeq: lastEventSeqRef.current,
            signal: abort.signal,
            onEvent: onStreamEvent
          });
          retryRef.current = 0;
        } catch (error) {
          if (abort.signal.aborted) {
            return;
          }

          if (isAuthError(error)) {
            setState((prev) => ({
              ...prev,
              connected: false,
              streaming: false,
              globalError: "认证失效，请重新连接。"
            }));
            return;
          }

          retryRef.current += 1;
          if (retryRef.current > MAX_RETRY) {
            setState((prev) => ({
              ...prev,
              streaming: false,
              globalError: "SSE 重连失败，请重新连接。"
            }));
            return;
          }

          setState((prev) => ({
            ...prev,
            info: `流中断，重连中 (${retryRef.current}/${MAX_RETRY})`
          }));
          await new Promise((resolve) => setTimeout(resolve, 300 * 2 ** retryRef.current));
        }
      }
    };

    void run();
  }, [client, onStreamEvent, stopStream]);

  const connect = useCallback(async () => {
    const baseUrl = state.baseUrl.trim();
    const ownerKey = state.ownerKey.trim();

    if (!baseUrl || !ownerKey) {
      setState((prev) => ({ ...prev, globalError: "请填写 Gateway 地址和 Owner Key。" }));
      return;
    }

    try {
      const loginClient = new GatewayClient({ baseUrl });
      const auth = await loginClient.login(ownerKey);
      const authedClient = new GatewayClient({ baseUrl, accessToken: auth.access_token });
      const session = await authedClient.createSession();

      lastEventSeqRef.current = 0;
      completedTurnsRef.current.clear();
      streamStore.clearSession(session.session_id);

      setState((prev) => ({
        ...prev,
        baseUrl,
        accessToken: auth.access_token,
        connected: true,
        sessionId: session.session_id,
        users: [],
        assistants: [],
        activeTurnId: null,
        globalError: null,
        info: "连接成功",
        replayExceeded: false
      }));

      startStreamLoop(session.session_id);
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error), connected: false }));
    }
  }, [startStreamLoop, state.baseUrl, state.ownerKey]);

  const send = useCallback(async (text: string) => {
    const content = text.trim();
    if (!content || !state.sessionId || !state.connected) {
      return;
    }

    const user: UserMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content,
      timestamp: new Date().toISOString()
    };

    setState((prev) => ({
      ...prev,
      users: [...prev.users, user],
      sending: true,
      globalError: null,
      info: null
    }));

    try {
      await client.submitTurn(state.sessionId, content);
      setState((prev) => ({ ...prev, sending: false }));
    } catch (error) {
      setState((prev) => ({ ...prev, sending: false, globalError: humanizeError(error) }));
    }
  }, [client, state.connected, state.sessionId]);

  const rebuildSession = useCallback(async () => {
    if (!state.connected) {
      return;
    }
    try {
      const session = await client.createSession();
      if (state.sessionId) {
        streamStore.clearSession(state.sessionId);
      }
      lastEventSeqRef.current = 0;
      completedTurnsRef.current.clear();
      setState((prev) => ({
        ...prev,
        sessionId: session.session_id,
        users: [],
        assistants: [],
        activeTurnId: null,
        globalError: null,
        info: "会话已重建",
        replayExceeded: false
      }));
      startStreamLoop(session.session_id);
    } catch (error) {
      setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
    }
  }, [client, startStreamLoop, state.connected, state.sessionId]);

  useEffect(() => () => {
    stopStream();
  }, [stopStream]);

  return {
    state,
    setBaseUrl: (value: string) => setState((prev) => ({ ...prev, baseUrl: value })),
    setOwnerKey: (value: string) => setState((prev) => ({ ...prev, ownerKey: value })),
    connect,
    send,
    rebuildSession,
    dismissError: () => setState((prev) => ({ ...prev, globalError: null })),
    dismissInfo: () => setState((prev) => ({ ...prev, info: null }))
  };
}
