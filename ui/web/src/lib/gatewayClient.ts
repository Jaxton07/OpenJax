import { parseGatewayError } from "./errors";
import type {
  AppSettings,
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

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
}

export class GatewayClient {
  private readonly settings: AppSettings;

  constructor(settings: AppSettings) {
    this.settings = settings;
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
    const query =
      options.afterEventSeq && options.afterEventSeq > 0
        ? `?after_event_seq=${options.afterEventSeq}`
        : "";

    const response = await fetch(
      `${normalizeBaseUrl(this.settings.baseUrl)}/api/v1/sessions/${options.sessionId}/events${query}`,
      {
        method: "GET",
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
      const chunks = buffer.split("\n\n");
      buffer = chunks.pop() ?? "";

      for (const chunk of chunks) {
        const parsed = parseSseChunk(chunk);
        if (!parsed?.data) {
          continue;
        }
        try {
          const event = JSON.parse(parsed.data) as StreamEvent;
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
    const apiKey = this.settings.apiKey.trim();
    return apiKey
      ? { Authorization: `Bearer ${apiKey}` }
      : {};
  }
}

function parseSseChunk(chunk: string): { event?: string; data?: string } | null {
  const lines = chunk
    .split("\n")
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
