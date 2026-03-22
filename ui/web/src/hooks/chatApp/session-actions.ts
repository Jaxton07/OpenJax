import { humanizeError, isAuthenticationError } from "../../lib/errors";
import { streamRenderStore } from "../../lib/streamRenderStore";
import type { GatewayClient } from "../../lib/gatewayClient";
import type { ChatSession, ChatState, PendingApproval } from "../../types/chat";
import { createLocalSession, humanizeProviderError, INFO_TOAST_ALREADY_IN_NEW_CHAT, isEmptyDraftSession, summarizeTitle } from "./session-model";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

type SetState = Dispatch<SetStateAction<ChatState>>;
type WithAuthRetry = <T>(action: () => Promise<T>) => Promise<T>;

interface EnsureSessionParams {
  activeSessionId: string | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  setState: SetState;
  clearAuthState: (message: string) => void;
}

export async function ensureSessionAction(params: EnsureSessionParams): Promise<string> {
  if (params.activeSessionId) {
    return params.activeSessionId;
  }

  params.setState((prev) => ({ ...prev, loading: true, globalError: null }));
  try {
    const created = await params.withAuthRetry(() => params.client.startSession());
    const session = createLocalSession(created.session_id);
    params.setState((prev) => ({
      ...prev,
      sessions: [session, ...prev.sessions],
      activeSessionId: session.id,
      loading: false
    }));
    return session.id;
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("认证失效，请重新登录。");
    } else {
      params.setState((prev) => ({ ...prev, loading: false, globalError: humanizeError(error) }));
    }
    throw error;
  }
}

interface SendMessageParams {
  content: string;
  ensureSession: () => Promise<string>;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  outputMode: ChatState["settings"]["outputMode"];
  pollingAbortRef: MutableRefObject<AbortController | null>;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function sendMessageAction(params: SendMessageParams): Promise<void> {
  const message = params.content.trim();
  if (!message) {
    return;
  }

  const sessionId = await params.ensureSession();
  params.updateSession(sessionId, (session) => ({
    ...session,
    turnPhase: "submitting",
    title: session.messages.length === 0 ? summarizeTitle(message) : session.title,
    messages: [
      ...session.messages,
      {
        id: crypto.randomUUID(),
        kind: "text",
        role: "user",
        content: message,
        timestamp: new Date().toISOString(),
        startEventSeq: session.lastEventSeq + 1,
        lastEventSeq: session.lastEventSeq + 1
      }
    ]
  }));

  try {
    const submitted = await params.withAuthRetry(() => params.client.submitTurn(sessionId, message));
    if (params.outputMode === "polling") {
      params.pollingAbortRef.current?.abort();
      const pollAbort = new AbortController();
      params.pollingAbortRef.current = pollAbort;

      params.updateSession(sessionId, (session) => ({ ...session, connection: "active", turnPhase: "streaming" }));
      const result = await params.withAuthRetry(() =>
        params.client.pollTurnUntilDone(sessionId, submitted.turn_id, pollAbort.signal)
      );

      if (result.status === "completed") {
        params.updateSession(sessionId, (session) => ({
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
              startEventSeq: session.lastEventSeq + 1,
              lastEventSeq: session.lastEventSeq + 1,
              turnId: result.turn_id
            }
          ]
        }));
      } else {
        params.updateSession(sessionId, (session) => ({
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
              startEventSeq: session.lastEventSeq + 1,
              lastEventSeq: session.lastEventSeq + 1,
              turnId: result.turn_id
            }
          ]
        }));
      }
    }
  } catch (error) {
    params.updateSession(sessionId, (session) => ({ ...session, turnPhase: "failed" }));
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeProviderError(error) }));
  }
}

interface NewChatParams {
  activeSession: ChatSession | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  setState: SetState;
  clearAuthState: (message: string) => void;
}

export async function newChatAction(params: NewChatParams): Promise<void> {
  if (isEmptyDraftSession(params.activeSession)) {
    params.setState((prev) => ({ ...prev, infoToast: INFO_TOAST_ALREADY_IN_NEW_CHAT, globalError: null }));
    return;
  }
  try {
    const created = await params.withAuthRetry(() => params.client.startSession());
    const session = createLocalSession(created.session_id);
    params.setState((prev) => ({
      ...prev,
      sessions: [session, ...prev.sessions],
      activeSessionId: session.id,
      globalError: null
    }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}

interface DeleteSessionParams {
  sessionId: string;
  activeSessionId: string | null;
  reconnectAbortRef: MutableRefObject<AbortController | null>;
  pollingAbortRef: MutableRefObject<AbortController | null>;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  setState: SetState;
  clearAuthState: (message: string) => void;
}

export async function deleteSessionAction(params: DeleteSessionParams): Promise<void> {
  const shouldDelete = window.confirm("确认删除该会话？此操作不可恢复。");
  if (!shouldDelete) {
    return;
  }

  if (params.activeSessionId === params.sessionId) {
    params.reconnectAbortRef.current?.abort();
    params.pollingAbortRef.current?.abort();
  }

  try {
    await params.withAuthRetry(() => params.client.shutdownSession(params.sessionId));
    streamRenderStore.clear(params.sessionId);
    params.setState((prev) => {
      const removedIndex = prev.sessions.findIndex((session) => session.id === params.sessionId);
      if (removedIndex < 0) {
        return prev;
      }

      const sessions = prev.sessions.filter((session) => session.id !== params.sessionId);
      const nextActive =
        prev.activeSessionId === params.sessionId
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
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}

interface ResolveApprovalParams {
  approval: PendingApproval;
  approved: boolean;
  activeSessionId: string | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function resolveApprovalAction(params: ResolveApprovalParams): Promise<void> {
  const sessionId = params.activeSessionId;
  if (!sessionId) {
    return;
  }
  try {
    await params.withAuthRetry(() =>
      params.client.resolveApproval(
        sessionId,
        params.approval.approvalId,
        params.approved,
        params.approved ? "approved" : "rejected"
      )
    );
    params.updateSession(sessionId, (session) => ({
      ...session,
      pendingApprovals: session.pendingApprovals.filter((item) => item.approvalId !== params.approval.approvalId)
    }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}

interface ClearConversationParams {
  activeSessionId: string | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function clearConversationAction(params: ClearConversationParams): Promise<void> {
  if (!params.activeSessionId) {
    return;
  }
  try {
    await params.withAuthRetry(() => params.client.clearSession(params.activeSessionId!));
    streamRenderStore.clear(params.activeSessionId);
    params.updateSession(params.activeSessionId, (session) => ({
      ...session,
      messages: [],
      pendingApprovals: [],
      turnPhase: "draft"
    }));
    params.setState((prev) => ({ ...prev, infoToast: "会话已清空" }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}

interface CompactConversationParams {
  activeSessionId: string | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function compactConversationAction(params: CompactConversationParams): Promise<void> {
  if (!params.activeSessionId) {
    return;
  }
  try {
    await params.withAuthRetry(() => params.client.compactSession(params.activeSessionId!));
    params.setState((prev) => ({ ...prev, infoToast: "会话已压缩" }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}
