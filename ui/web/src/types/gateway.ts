export type OutputMode = "sse" | "polling";

export interface AppSettings {
  baseUrl: string;
  outputMode: OutputMode;
}

export interface AuthState {
  apiKey: string;
  authenticated: boolean;
}

export interface GatewayConnection {
  baseUrl: string;
  apiKey: string;
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

export interface StreamEvent {
  request_id: string;
  session_id: string;
  turn_id?: string;
  event_seq: number;
  timestamp: string;
  type:
    | "turn_started"
    | "assistant_delta"
    | "assistant_message"
    | "tool_call_started"
    | "tool_call_completed"
    | "approval_requested"
    | "approval_resolved"
    | "turn_completed"
    | "session_shutdown"
    | "error";
  payload: Record<string, unknown>;
}
