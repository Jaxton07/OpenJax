export type OutputMode = "sse" | "polling";

export interface AppSettings {
  baseUrl: string;
  outputMode: OutputMode;
}

export interface AuthState {
  authenticated: boolean;
  accessToken: string;
  sessionId: string | null;
  scope: string | null;
}

export interface GatewayConnection {
  baseUrl: string;
  accessToken: string;
}

export interface GatewayErrorBody {
  code: string;
  message: string;
  retryable: boolean;
  details: Record<string, unknown>;
}

export interface GatewayErrorEnvelope {
  request_id: string;
  timestamp: string;
  error: GatewayErrorBody;
}

export interface GatewayError extends Error {
  status: number;
  code: string;
  retryable: boolean;
}

export interface SessionCreated {
  request_id: string;
  session_id: string;
  timestamp: string;
}

export interface TurnSubmitted {
  request_id: string;
  session_id: string;
  turn_id: string;
  timestamp: string;
}

export interface TurnStatusResponse {
  request_id: string;
  session_id: string;
  turn_id: string;
  status: "queued" | "running" | "completed" | "failed";
  assistant_message?: string;
  error?: GatewayErrorBody;
  timestamp: string;
}

export interface SessionActionResponse {
  request_id: string;
  session_id: string;
  status: "cleared" | "shutdown" | "resolved";
  approval_id?: string;
  timestamp: string;
}

export interface AuthLoginResponse {
  request_id: string;
  access_token: string;
  access_expires_in: number;
  session_id: string;
  scope: string;
  timestamp: string;
}

export interface AuthRevokeResponse {
  request_id: string;
  revoked: number;
  timestamp: string;
}

export interface AuthSessionItem {
  session_id: string;
  device_id: string;
  scope: string;
  device_name?: string;
  platform?: string;
  user_agent?: string;
  status: string;
  created_at: string;
  last_seen_at: string;
  revoked_at?: string | null;
}

export interface AuthSessionsResponse {
  request_id: string;
  sessions: AuthSessionItem[];
  timestamp: string;
}

export interface StreamEvent {
  request_id: string;
  session_id: string;
  turn_id?: string;
  event_seq: number;
  turn_seq?: number;
  timestamp: string;
  stream_source?: "model_live" | "synthetic" | "replay" | "unknown";
  type:
    | "turn_started"
    | "assistant_delta"
    | "assistant_message"
    | "response_started"
    | "response_text_delta"
    | "tool_calls_proposed"
    | "tool_batch_completed"
    | "response_resumed"
    | "response_completed"
    | "response_error"
    | "tool_call_started"
    | "tool_call_completed"
    | "approval_requested"
    | "approval_resolved"
    | "turn_completed"
    | "session_shutdown"
    | "error";
  payload: Record<string, unknown>;
}
