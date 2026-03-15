import type { AppSettings, AuthState, StreamEvent } from "./gateway";

export type SessionConnection = "idle" | "connecting" | "active" | "closing" | "closed";
export type TurnPhase = "draft" | "submitting" | "streaming" | "completed" | "failed";
export type MessageRole = "user" | "assistant" | "tool" | "error" | "system";
export type MessageKind = "text" | "tool_steps";
export type ToolStepStatus = "running" | "success" | "waiting" | "failed";
export type ToolStepType = "think" | "tool" | "shell" | "approval" | "summary";

export interface ReasoningBlock {
  blockId: string;
  turnId: string;
  content: string;
  collapsed: boolean;
  startedAt: string;
  closed: boolean;
}

export interface ToolStep {
  id: string;
  type: ToolStepType;
  title: string;
  status: ToolStepStatus;
  time: string;
  startedAt?: string;
  endedAt?: string;
  durationSec?: number;
  subtitle?: string;
  description?: string;
  code?: string;
  output?: string;
  delta?: string;
  approvalId?: string;
  toolCallId?: string;
  meta?: Record<string, unknown>;
}

export interface ChatMessage {
  id: string;
  kind: MessageKind;
  role: MessageRole;
  content: string;
  timestamp: string;
  turnId?: string;
  isDraft?: boolean;
  hasCanonicalDelta?: boolean;
  toolSteps?: ToolStep[];
  reasoningBlocks?: ReasoningBlock[];
}

export interface PendingApproval {
  approvalId: string;
  toolCallId?: string;
  turnId?: string;
  target?: string;
  reason?: string;
  toolName?: string;
}

export interface SessionStreamingState {
  turnId?: string;
  assistantMessageId?: string;
  content: string;
  lastEventSeq: number;
  active: boolean;
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
  streaming?: SessionStreamingState;
}

export interface ChatState {
  settings: AppSettings;
  auth: AuthState;
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
