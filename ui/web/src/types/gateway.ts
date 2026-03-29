export type OutputMode = "sse" | "polling";

export type ProviderProtocol = "chat_completions" | "anthropic_messages";

export interface AppSettings {
  baseUrl: string;
  outputMode: OutputMode;
  selectedProviderId: string | null;
  selectedModelName: string | null;
}

export interface LlmProvider {
  provider_id: string;
  provider_name: string;
  base_url: string;
  model_name: string;
  api_key_set: boolean;
  created_at: string;
  updated_at: string;
  provider_type: "built_in" | "custom";
  protocol: ProviderProtocol;
  context_window_size: number;
}

export interface CatalogModel {
  model_id: string;
  display_name: string;
  context_window: number;
}

export interface CatalogProvider {
  catalog_key: string;
  display_name: string;
  base_url: string;
  protocol: ProviderProtocol;
  default_model: string;
  models: CatalogModel[];
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

export interface GatewaySessionSummary {
  session_id: string;
  title?: string;
  created_at: string;
  updated_at: string;
}

export interface GatewaySessionListResponse {
  request_id: string;
  sessions: GatewaySessionSummary[];
  timestamp: string;
}

export interface GatewaySessionMessage {
  message_id: string;
  session_id: string;
  turn_id?: string;
  role: string;
  content: string;
  sequence: number;
  created_at: string;
}

export interface GatewaySessionMessagesResponse {
  request_id: string;
  session_id: string;
  messages: GatewaySessionMessage[];
  timestamp: string;
}

export interface GatewaySessionTimelineResponse {
  request_id: string;
  session_id: string;
  events: StreamEvent[];
  timestamp: string;
}

export interface ShellExecutionMetadata {
  result_class: string;
  backend: string;
  exit_code: number;
  policy_decision: string;
  runtime_allowed: boolean;
  degrade_reason?: string | null;
  runtime_deny_reason?: string | null;
}

export interface ToolCallStartedPayload extends Record<string, unknown> {
  tool_call_id?: string;
  tool_name?: string;
  target?: string;
  display_name?: string;
}

export interface ToolCallCompletedPayload extends Record<string, unknown> {
  tool_call_id?: string;
  tool_name?: string;
  display_name?: string;
  ok?: boolean;
  output?: string;
  shell_metadata?: ShellExecutionMetadata;
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
  /** @deprecated Legacy compat only. Prefer response_completed events. */
  assistant_message?: string;
  error?: GatewayErrorBody;
  timestamp: string;
}

export interface SessionActionResponse {
  request_id: string;
  session_id: string;
  status: "cleared" | "shutdown" | "resolved" | "compacted" | "aborted";
  approval_id?: string;
  timestamp: string;
}

export interface ProviderListResponse {
  request_id: string;
  providers: LlmProvider[];
  timestamp: string;
}

export interface ProviderMutationResponse {
  request_id: string;
  provider: LlmProvider;
  timestamp: string;
}

export interface ProviderDeleteResponse {
  request_id: string;
  provider_id: string;
  status: "deleted";
  timestamp: string;
}

export interface ActiveProvider {
  provider_id: string;
  model_name: string;
  updated_at: string;
}

export interface ActiveProviderResponse {
  request_id: string;
  active_provider?: ActiveProvider;
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
    | "user_message"
    | "turn_started"
    | AssistantMessageCompatType
    | "response_started"
    | "reasoning_delta"
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
    | "context_usage_updated"
    | "turn_completed"
    | "turn_interrupted"
    | "session_shutdown"
    | "error";
  payload: Record<string, unknown>;
}

/** @deprecated Legacy compat only. Prefer response_completed events. */
export type AssistantMessageCompatType = "assistant_message";

export interface SlashCommandDto {
  name: string;
  aliases: string[];
  description: string;
  usage_hint: string;
  kind: "builtin" | "session_action" | "skill" | "local_picker";
  replaces_input: boolean;
}

export interface SlashCommandsResponse {
  commands: SlashCommandDto[];
}

export interface SlashExecRequest {
  command: string;
}

export interface SlashExecResponse {
  status: "ok" | "pending" | string;
  message?: string;
  action?: string | null;
}

export interface GetPolicyLevelResponse {
  session_id: string;
  level: "allow" | "ask" | "deny";
}
