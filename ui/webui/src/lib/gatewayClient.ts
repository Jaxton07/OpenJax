import { fetchEventSource } from "@microsoft/fetch-event-source";
import { parseGatewayError } from "./errors";
import type {
  AuthLoginResponse,
  GatewayConnection,
  SessionCreated,
  StreamEvent,
  TurnSubmitted
} from "../types/gateway";

interface StreamEventsOptions {
  sessionId: string;
  afterEventSeq?: number;
  signal: AbortSignal;
  onEvent: (event: StreamEvent) => void;
}

export class GatewayClient {
  private readonly connection: GatewayConnection;

  constructor(connection: GatewayConnection) {
    this.connection = connection;
  }

  async login(ownerKey: string): Promise<AuthLoginResponse> {
    return this.request("/api/v1/auth/login", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${ownerKey.trim()}`
      },
      body: JSON.stringify({
        device_name: "openjax-webui",
        platform: "web",
        user_agent: typeof navigator !== "undefined" ? navigator.userAgent : "unknown"
      })
    });
  }

  async createSession(): Promise<SessionCreated> {
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

  async streamEvents(options: StreamEventsOptions): Promise<void> {
    const params = new URLSearchParams();
    if (options.afterEventSeq && options.afterEventSeq > 0) {
      params.set("after_event_seq", String(options.afterEventSeq));
    }
    params.set("protocol", "v2");

    const query = params.toString().length > 0 ? `?${params.toString()}` : "";
    const url = `${normalizeBaseUrl(this.connection.baseUrl)}/api/v1/sessions/${options.sessionId}/events${query}`;

    await fetchEventSource(url, {
      method: "GET",
      signal: options.signal,
      headers: {
        ...this.authHeader()
      },
      credentials: "include",
      async onopen(response) {
        if (response.ok) {
          return;
        }
        throw await parseGatewayError(response);
      },
      onmessage(msg) {
        if (!msg.data) {
          return;
        }
        const event = parseSseData(msg.data);
        if (!event) {
          return;
        }
        options.onEvent(event);
      },
      onerror(error) {
        throw error;
      }
    });
  }

  private async request<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${normalizeBaseUrl(this.connection.baseUrl)}${path}`, {
      ...init,
      credentials: "include",
      headers: {
        "Content-Type": "application/json",
        ...this.authHeader(),
        ...(init.headers ?? {})
      }
    });

    if (!response.ok) {
      throw await parseGatewayError(response);
    }

    return (await response.json()) as T;
  }

  private authHeader(): Record<string, string> {
    const token = (this.connection.accessToken ?? "").trim();
    if (!token) {
      return {};
    }
    return {
      Authorization: `Bearer ${token}`
    };
  }
}

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
}

export function parseSseData(data: string): StreamEvent | null {
  try {
    return JSON.parse(data) as StreamEvent;
  } catch {
    return null;
  }
}
