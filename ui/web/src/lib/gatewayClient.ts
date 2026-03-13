import { parseGatewayError } from "./errors";
import type {
  AuthLoginResponse,
  AuthRevokeResponse,
  AuthSessionsResponse,
  GatewayConnection,
  SessionActionResponse,
  SessionCreated,
  StreamEvent,
  TurnStatusResponse,
  TurnSubmitted
} from "../types/gateway";

interface StreamOptions {
  sessionId: string;
  afterEventSeq?: number;
  onEvent: (event: StreamEvent) => void;
  onError: (error: Error) => void;
  signal: AbortSignal;
}

const STREAM_DEBUG_ENABLED = resolveStreamDebugEnabled();

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
}

export class GatewayClient {
  private readonly settings: GatewayConnection;

  constructor(settings: GatewayConnection) {
    this.settings = settings;
  }

  async login(ownerKey: string): Promise<AuthLoginResponse> {
    return this.request("/api/v1/auth/login", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${ownerKey.trim()}`
      },
      body: JSON.stringify({
        device_name: "openjax-web",
        platform: "web",
        user_agent: typeof navigator !== "undefined" ? navigator.userAgent : "unknown"
      })
    });
  }

  async refresh(refreshToken?: string): Promise<AuthLoginResponse> {
    return this.request("/api/v1/auth/refresh", {
      method: "POST",
      body: JSON.stringify(refreshToken ? { refresh_token: refreshToken } : {})
    });
  }

  async logout(sessionId: string): Promise<{ status: string }> {
    return this.request("/api/v1/auth/logout", {
      method: "POST",
      body: JSON.stringify({ session_id: sessionId })
    });
  }

  async revoke(payload: {
    sessionId?: string;
    deviceId?: string;
    revokeAll?: boolean;
  }): Promise<AuthRevokeResponse> {
    return this.request("/api/v1/auth/revoke", {
      method: "POST",
      body: JSON.stringify({
        session_id: payload.sessionId,
        device_id: payload.deviceId,
        revoke_all: payload.revokeAll ?? false
      })
    });
  }

  async listSessions(): Promise<AuthSessionsResponse> {
    return this.request("/api/v1/auth/sessions", {
      method: "GET"
    });
  }

  async startSession(): Promise<SessionCreated> {
    return this.request("/api/v1/sessions", {
      method: "POST",
      body: JSON.stringify({})
    });
  }

  async submitTurn(sessionId: string, input: string): Promise<TurnSubmitted> {
    return this.request(`/api/v1/sessions/${sessionId}/turns`, {
      method: "POST",
      body: JSON.stringify({ input })
    });
  }

  async getTurn(sessionId: string, turnId: string): Promise<TurnStatusResponse> {
    return this.request(`/api/v1/sessions/${sessionId}/turns/${turnId}`, {
      method: "GET"
    });
  }

  async resolveApproval(
    sessionId: string,
    approvalId: string,
    approved: boolean,
    reason?: string
  ): Promise<SessionActionResponse> {
    return this.request(`/api/v1/sessions/${sessionId}/approvals/${approvalId}:resolve`, {
      method: "POST",
      body: JSON.stringify({ approved, reason })
    });
  }

  async clearSession(sessionId: string): Promise<SessionActionResponse> {
    return this.request(`/api/v1/sessions/${sessionId}:clear`, {
      method: "POST",
      body: JSON.stringify({ reason: "user requested clear" })
    });
  }

  async compactSession(sessionId: string): Promise<SessionActionResponse> {
    return this.request(`/api/v1/sessions/${sessionId}:compact`, {
      method: "POST",
      body: JSON.stringify({ strategy: "default" })
    });
  }

  async shutdownSession(sessionId: string): Promise<SessionActionResponse> {
    return this.request(`/api/v1/sessions/${sessionId}`, {
      method: "DELETE"
    });
  }

  async healthCheck(): Promise<{ status: string }> {
    return this.request("/healthz", { method: "GET" });
  }

  async pollTurnUntilDone(
    sessionId: string,
    turnId: string,
    signal: AbortSignal,
    onTick?: (state: TurnStatusResponse) => void
  ): Promise<TurnStatusResponse> {
    while (!signal.aborted) {
      const state = await this.getTurn(sessionId, turnId);
      onTick?.(state);
      if (state.status === "completed" || state.status === "failed") {
        return state;
      }
      await sleep(900, signal);
    }
    throw new Error("polling aborted");
  }

  async streamEvents(options: StreamOptions): Promise<void> {
    const params = new URLSearchParams();
    if (options.afterEventSeq && options.afterEventSeq > 0) {
      params.set("after_event_seq", String(options.afterEventSeq));
    }
    params.set("protocol", "v2");
    const query = params.toString().length > 0 ? `?${params.toString()}` : "";

    const response = await fetch(
      `${normalizeBaseUrl(this.settings.baseUrl)}/api/v1/sessions/${options.sessionId}/events${query}`,
      {
        method: "GET",
        credentials: "include",
        headers: this.headers(),
        signal: options.signal
      }
    );

    if (!response.ok) {
      throw await parseGatewayError(response);
    }

    const body = response.body;
    if (!body) {
      throw new Error("SSE stream body unavailable");
    }

    const reader = body.getReader();
    const decoder = new TextDecoder("utf-8");
    let buffer = "";

    while (!options.signal.aborted) {
      const result = await reader.read();
      if (result.done) {
        break;
      }
      buffer += decoder.decode(result.value, { stream: true });
      const split = splitSseBuffer(buffer);
      buffer = split.remainder;

      for (const chunk of split.chunks) {
        const parsed = parseSseChunk(chunk);
        if (!parsed?.data) {
          continue;
        }
        try {
          const event = JSON.parse(parsed.data) as StreamEvent;
          if (STREAM_DEBUG_ENABLED) {
            console.debug("[stream_debug][gateway_client][recv]", {
              sessionId: options.sessionId,
              eventType: event.type,
              eventSeq: event.event_seq,
              turnId: event.turn_id,
              turnSeq: event.turn_seq,
              deltaLen:
                event.type === "response_text_delta"
                  ? String(event.payload.content_delta ?? "").length
                  : undefined
            });
          }
          options.onEvent(event);
        } catch (error) {
          options.onError(error as Error);
        }
      }
    }
  }

  private async request<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${normalizeBaseUrl(this.settings.baseUrl)}${path}`, {
      ...init,
      credentials: "include",
      headers: {
        "Content-Type": "application/json",
        ...this.headers(),
        ...(init.headers ?? {})
      }
    });

    if (!response.ok) {
      throw await parseGatewayError(response);
    }

    return (await response.json()) as T;
  }

  private headers(): Record<string, string> {
    const accessToken = this.settings.accessToken.trim();
    return accessToken ? { Authorization: `Bearer ${accessToken}` } : {};
  }
}

export function splitSseBuffer(buffer: string): { chunks: string[]; remainder: string } {
  const normalized = buffer.replace(/\r\n/g, "\n");
  const chunks = normalized.split("\n\n");
  return {
    chunks: chunks.slice(0, -1),
    remainder: chunks.at(-1) ?? ""
  };
}

function parseSseChunk(chunk: string): { event?: string; data?: string } | null {
  const lines = chunk
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  if (lines.length === 0) {
    return null;
  }

  const output: { event?: string; data?: string } = {};
  for (const line of lines) {
    if (line.startsWith("event:")) {
      output.event = line.slice("event:".length).trim();
    }
    if (line.startsWith("data:")) {
      output.data = line.slice("data:".length).trim();
    }
  }
  return output;
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => resolve(), ms);
    signal.addEventListener("abort", () => {
      clearTimeout(timer);
      reject(new Error("aborted"));
    });
  });
}

function resolveStreamDebugEnabled(): boolean {
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
          VITE_OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
        })
      : {};
  const raw = String(
    globals.OPENJAX_WEB_STREAM_DEBUG ??
      globals.VITE_OPENJAX_WEB_STREAM_DEBUG ??
      "0"
  )
    .trim()
    .toLowerCase();
  return !(raw === "0" || raw === "off" || raw === "false" || raw === "disabled");
}
