export interface GatewayConnection {
  baseUrl: string;
  accessToken?: string;
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

export interface AuthLoginResponse {
  request_id: string;
  access_token: string;
  access_expires_in: number;
  session_id: string;
  scope: string;
  timestamp: string;
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

export type StreamEventType =
  | "response_started"
  | "response_text_delta"
  | "response_completed"
  | "assistant_message"
  | "response_error"
  | "tool_call_ready"
  | "turn_completed"
  | "error"
  | string;

export interface StreamEvent {
  request_id: string;
  session_id: string;
  turn_id?: string;
  event_seq: number;
  turn_seq?: number;
  timestamp: string;
  type: StreamEventType;
  stream_source?: "model_live" | "synthetic" | "replay" | "unknown";
  payload: Record<string, unknown>;
}
