import { humanizeError, isAuthenticationError } from "../../lib/errors";
import { streamRenderStore } from "../../lib/streamRenderStore";
import type { GatewayClient } from "../../lib/gatewayClient";
import type { ChatSession, ChatState, PendingApproval } from "../../types/chat";
import {
  createLocalSession,
  humanizeProviderError,
  INFO_TOAST_ALREADY_IN_NEW_CHAT,
  isEmptyDraftSession,
  PLACEHOLDER_SESSION_TITLE,
  summarizeTitle
} from "./session-model";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

type SetState = Dispatch<SetStateAction<ChatState>>;
type WithAuthRetry = <T>(action: () => Promise<T>) => Promise<T>;

interface EnsureSessionParams {
  activeSessionId: string | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  setState: SetState;
  clearAuthState: (message: string) => void;
  shouldActivateCreatedSession?: () => boolean;
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
      activeSessionId: params.shouldActivateCreatedSession?.() === false ? prev.activeSessionId : session.id,
      loading: false
    }));
    return session.id;
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.setState((prev) => ({ ...prev, loading: false }));
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
  getSessionTurnPhase?: (sessionId: string) => ChatSession["turnPhase"] | undefined;
  getSessionTitle?: (sessionId: string) => string | undefined;
  getSessionIsPlaceholderTitle?: (sessionId: string) => boolean | undefined;
  getSessionMessageCount?: (sessionId: string) => number | undefined;
  tryBeginSubmit?: (sessionId: string) => boolean;
  endSubmit?: (sessionId: string) => void;
  notifyBusyTurnBlockedSend?: () => void;
}

export async function sendMessageAction(params: SendMessageParams): Promise<void> {
  const message = params.content.trim();
  if (!message) {
    return;
  }

  const sessionId = await params.ensureSession();
  const gateAccepted = params.tryBeginSubmit ? params.tryBeginSubmit(sessionId) : true;
  if (!gateAccepted) {
    params.notifyBusyTurnBlockedSend?.();
    return;
  }
  const priorTurnPhase = params.getSessionTurnPhase?.(sessionId);
  const priorTitle = params.getSessionTitle?.(sessionId);
  const priorIsPlaceholderTitle = params.getSessionIsPlaceholderTitle?.(sessionId);
  const priorMessageCount = params.getSessionMessageCount?.(sessionId) ?? 0;
  const optimisticTitle = priorMessageCount === 0 ? summarizeTitle(message) : undefined;
  if (isBusyTurnPhase(priorTurnPhase)) {
    params.notifyBusyTurnBlockedSend?.();
    params.endSubmit?.(sessionId);
    return;
  }
  const optimisticMessageId = crypto.randomUUID();
  params.updateSession(sessionId, (session) => ({
    ...session,
    turnPhase: "submitting",
    title: session.messages.length === 0 ? (optimisticTitle ?? session.title) : session.title,
    isPlaceholderTitle: session.messages.length === 0 ? false : session.isPlaceholderTitle,
    messages: [
      ...session.messages,
      {
        id: optimisticMessageId,
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
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    if (isGatewayConflictError(error)) {
      params.updateSession(sessionId, (session) => ({
        ...session,
        turnPhase: session.turnPhase === "submitting" ? (priorTurnPhase ?? "draft") : session.turnPhase,
        title:
          priorMessageCount === 0 && optimisticTitle && session.title === optimisticTitle
            ? (priorTitle ?? session.title)
            : session.title,
        isPlaceholderTitle:
          priorMessageCount === 0 && optimisticTitle && session.title === optimisticTitle
            ? (priorIsPlaceholderTitle ?? session.isPlaceholderTitle)
            : session.isPlaceholderTitle,
        messages: session.messages.filter((item) => item.id !== optimisticMessageId)
      }));
      params.notifyBusyTurnBlockedSend?.();
      return;
    }
    params.updateSession(sessionId, (session) => ({ ...session, turnPhase: "failed" }));
    params.setState((prev) => ({ ...prev, globalError: humanizeProviderError(error) }));
  } finally {
    params.endSubmit?.(sessionId);
  }
}

function isBusyTurnPhase(phase: ChatSession["turnPhase"] | undefined): boolean {
  return phase === "submitting" || phase === "streaming";
}

function isGatewayConflictError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const gateway = error as Partial<{ code: string; status: number }>;
  return gateway.code === "CONFLICT" || gateway.status === 409;
}

interface NewChatParams {
  activeSession: ChatSession | null;
  withAuthRetry: WithAuthRetry;
  client: GatewayClient;
  setState: SetState;
  clearAuthState: (message: string) => void;
}

export async function newChatAction(params: NewChatParams): Promise<void> {
  if (!params.activeSession || isEmptyDraftSession(params.activeSession)) {
    params.setState((prev) => ({ ...prev, infoToast: INFO_TOAST_ALREADY_IN_NEW_CHAT, globalError: null }));
    return;
  }
  params.setState((prev) => ({
    ...prev,
    activeSessionId: null,
    globalError: null,
    infoToast: null
  }));
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
      title: PLACEHOLDER_SESSION_TITLE,
      isPlaceholderTitle: true,
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

interface FetchPolicyLevelParams {
  client: GatewayClient;
  sessionId: string;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
}

export async function fetchPolicyLevelAction(params: FetchPolicyLevelParams): Promise<void> {
  try {
    const response = await params.client.getPolicyLevel(params.sessionId);
    params.updateSession(params.sessionId, (session) => ({
      ...session,
      policyLevel: response.level,
    }));
  } catch {
    // Silent fallback — GET failure leaves policyLevel undefined, UI uses "ask"
  }
}

interface ChangePolicyLevelParams {
  client: GatewayClient;
  sessionId: string;
  level: "allow" | "ask" | "deny";
  withAuthRetry: WithAuthRetry;
  updateSession: (sessionId: string, updater: (session: ChatSession) => ChatSession) => void;
  clearAuthState: (message: string) => void;
  setState: SetState;
}

export async function changePolicyLevelAction(params: ChangePolicyLevelParams): Promise<void> {
  try {
    await params.withAuthRetry(() => params.client.setPolicyLevel(params.sessionId, params.level));
    params.updateSession(params.sessionId, (session) => ({
      ...session,
      policyLevel: params.level,
    }));
  } catch (error) {
    if (isAuthenticationError(error)) {
      params.clearAuthState("登录态已失效，请重新登录。");
      return;
    }
    params.setState((prev) => ({ ...prev, globalError: humanizeError(error) }));
  }
}
