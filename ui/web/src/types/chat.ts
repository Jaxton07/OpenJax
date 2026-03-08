import type { AppSettings, StreamEvent } from "./gateway";

export type SessionConnection = "idle" | "connecting" | "active" | "closing" | "closed";
export type TurnPhase = "draft" | "submitting" | "streaming" | "completed" | "failed";
export type MessageRole = "user" | "assistant" | "tool" | "error" | "system";

export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  turnId?: string;
  isDraft?: boolean;
}

export interface PendingApproval {
  approvalId: string;
  turnId?: string;
  target?: string;
  reason?: string;
  toolName?: string;
}

export interface ChatSession {
  id: string;
  title: string;
  createdAt: string;
  connection: SessionConnection;
  turnPhase: TurnPhase;
  lastEventSeq: number;
  messages: ChatMessage[];
  pendingApprovals: PendingApproval[];
}

export interface ChatState {
  settings: AppSettings;
  sessions: ChatSession[];
  activeSessionId: string | null;
  globalError: string | null;
  infoToast: string | null;
  loading: boolean;
}

export interface StreamEnvelope {
  event: StreamEvent;
  reconnecting?: boolean;
}
