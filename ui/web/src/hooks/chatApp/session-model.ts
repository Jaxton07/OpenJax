import { humanizeError } from "../../lib/errors";
import { applySessionEvents } from "../../lib/session-events/reducer";
import type { ChatMessage, ChatSession, MessageRole } from "../../types/chat";
import type { GatewayError, GatewaySessionSummary, StreamEvent } from "../../types/gateway";

export const INFO_TOAST_ALREADY_IN_NEW_CHAT = "已在新对话中";
export const PLACEHOLDER_SESSION_TITLE = "新聊天";
export const SESSION_TITLE_MAX_CODE_POINTS = 24;

interface ResolvedSessionTitle {
  title: string;
  isPlaceholderTitle: boolean;
}

export function createLocalSession(sessionId: string): ChatSession {
  return {
    id: sessionId,
    title: PLACEHOLDER_SESSION_TITLE,
    isPlaceholderTitle: true,
    createdAt: new Date().toISOString(),
    connection: "idle",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: []
  };
}

export function summarizeTitle(input: string): string {
  const plain = input.replace(/\s+/g, " ").trim();
  const codePoints = Array.from(plain);
  return codePoints.length > SESSION_TITLE_MAX_CODE_POINTS
    ? `${codePoints.slice(0, SESSION_TITLE_MAX_CODE_POINTS).join("")}...`
    : plain;
}

export function resolveSessionTitle(params: {
  remoteTitle?: string | null;
  localTitle?: string | null;
  localIsPlaceholderTitle?: boolean;
  inferredTitle?: string | null;
}): ResolvedSessionTitle {
  const remote = params.remoteTitle?.trim();
  if (remote) {
    return { title: remote, isPlaceholderTitle: false };
  }
  const local = params.localTitle?.trim();
  if (local && params.localIsPlaceholderTitle === false) {
    return { title: local, isPlaceholderTitle: false };
  }
  const inferred = params.inferredTitle?.trim();
  if (inferred) {
    return { title: inferred, isPlaceholderTitle: false };
  }
  return { title: PLACEHOLDER_SESSION_TITLE, isPlaceholderTitle: true };
}

export function isEmptyDraftSession(session: ChatSession | null): boolean {
  if (!session) {
    return false;
  }
  return (
    session.turnPhase === "draft" &&
    session.messages.length === 0 &&
    session.pendingApprovals.length === 0
  );
}

export function mapGatewayRoleToMessageRole(role: string): MessageRole {
  if (role === "user" || role === "assistant" || role === "tool" || role === "error" || role === "system") {
    return role;
  }
  return "system";
}

export function buildChatSessionFromGateway(
  remoteSession: GatewaySessionSummary,
  remoteEvents: StreamEvent[]
): ChatSession {
  const orderedEvents = [...remoteEvents].sort((left, right) => left.event_seq - right.event_seq);
  let session: ChatSession = {
    id: remoteSession.session_id,
    ...resolveSessionTitle({ remoteTitle: remoteSession.title }),
    createdAt: remoteSession.created_at,
    connection: "idle",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: []
  };
  if (orderedEvents.length > 0) {
    session = applySessionEvents(session, orderedEvents);
  }
  const messages: ChatMessage[] = session.messages;
  const firstUserMessage = messages.find((message) => message.role === "user");
  const resolved = resolveSessionTitle({
    remoteTitle: remoteSession.title,
    localTitle: session.title,
    localIsPlaceholderTitle: session.isPlaceholderTitle,
    inferredTitle: summarizeTitle(firstUserMessage?.content ?? "")
  });
  return {
    ...session,
    ...resolved,
    connection: "idle",
    createdAt: remoteSession.created_at
  };
}

export function isSessionNotFoundError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const gateway = error as Partial<GatewayError>;
  return gateway.code === "NOT_FOUND" || gateway.status === 404;
}

export function humanizeProviderError(error: unknown): string {
  const normalized = humanizeError(error);
  if (!error || typeof error !== "object" || !("code" in error)) {
    return normalized;
  }
  const gateway = error as GatewayError;
  if (gateway.code !== "UPSTREAM_UNAVAILABLE" && gateway.status !== 404) {
    return normalized;
  }
  const detail = [gateway.status ? `HTTP ${gateway.status}` : "", gateway.message]
    .filter((part) => part && part.trim().length > 0)
    .join(" - ");
  if (!detail || normalized.includes(detail)) {
    return normalized;
  }
  return `${normalized}（${detail}）`;
}
