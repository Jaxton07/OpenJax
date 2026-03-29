import type { AppSettings, AuthState, ShellExecutionMetadata, StreamEvent } from "./gateway";

export type SessionConnection = "idle" | "connecting" | "active" | "closing" | "closed";
export type TurnPhase = "draft" | "submitting" | "streaming" | "completed" | "failed";
export type MessageRole = "user" | "assistant" | "tool" | "error" | "system";
export type MessageKind = "text" | "tool_steps";
export type ToolStepStatus = "running" | "success" | "waiting" | "failed";
export type ToolStepType = "think" | "tool" | "shell" | "approval" | "summary";

export interface ToolStepMeta {
  rawPayload?: Record<string, unknown>;
  shellMetadata?: ShellExecutionMetadata;
  backendSummary?: string;
  riskSummary?: string;
  hint?: string;
  partial?: boolean;
}

export interface ReasoningBlock {
  blockId: string;
  turnId: string;
  content: string;
  collapsed: boolean;
  startedAt: string;
  endedAt?: string;
  closed: boolean;
  startEventSeq?: number;
  lastEventSeq?: number;
  endEventSeq?: number;
}

export interface ToolStep {
  id: string;
  type: ToolStepType;
  title: string;
  status: ToolStepStatus;
  time: string;
  startEventSeq?: number;
  lastEventSeq?: number;
  endEventSeq?: number;
  startedAt?: string;
  endedAt?: string;
  durationSec?: number;
  subtitle?: string;
  description?: string;
  code?: string;
  output?: string;
  delta?: string;
  target?: string;
  approvalId?: string;
  toolCallId?: string;
  meta?: ToolStepMeta;
}

export interface ChatMessage {
  id: string;
  kind: MessageKind;
  role: MessageRole;
  content: string;
  timestamp: string;
  startEventSeq?: number;
  lastEventSeq?: number;
  textStartEventSeq?: number;
  textLastEventSeq?: number;
  textEndEventSeq?: number;
  turnId?: string;
  isDraft?: boolean;
  hasCanonicalDelta?: boolean;
  interrupted?: boolean;
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

export interface ContextUsageState {
  ratio: number;
  inputTokens: number;
  contextWindowSize: number;
  updatedAt: string;
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
  contextUsage?: ContextUsageState;
  streaming?: SessionStreamingState;
  policyLevel?: "allow" | "ask" | "deny";
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
