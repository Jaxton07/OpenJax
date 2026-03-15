import type { ChatSession } from "../types/chat";
import type { AppSettings, AuthState } from "../types/gateway";

const SETTINGS_KEY = "openjax:web:settings";
const AUTH_KEY = "openjax:web:auth";
const SESSIONS_KEY = "openjax:web:sessions";

const DEFAULT_SETTINGS: AppSettings = {
  baseUrl: "http://127.0.0.1:8765",
  outputMode: "sse"
};

const DEFAULT_AUTH: AuthState = {
  authenticated: false,
  accessToken: "",
  sessionId: null,
  scope: null
};

function normalizeBaseUrl(baseUrl: string): string {
  const normalized = baseUrl.trim();
  if (normalized === "http://127.0.0.1:8080") {
    return DEFAULT_SETTINGS.baseUrl;
  }
  const localhostMatch = normalized.match(/^http:\/\/localhost(?::(\d+))?$/i);
  if (localhostMatch) {
    const port = localhostMatch[1] ?? "8765";
    return `http://127.0.0.1:${port}`;
  }
  return normalized;
}

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) {
      return DEFAULT_SETTINGS;
    }
    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    if (!parsed.baseUrl || !parsed.outputMode) {
      return DEFAULT_SETTINGS;
    }
    const baseUrl = normalizeBaseUrl(parsed.baseUrl);
    return {
      baseUrl,
      outputMode: parsed.outputMode
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: AppSettings): void {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
}

export function loadAuth(): AuthState {
  try {
    const raw = localStorage.getItem(AUTH_KEY);
    if (!raw) {
      return DEFAULT_AUTH;
    }
    const parsed = JSON.parse(raw) as Partial<AuthState>;
    if (!parsed.authenticated) {
      return DEFAULT_AUTH;
    }
    return {
      authenticated: true,
      accessToken: "",
      sessionId: null,
      scope: null
    };
  } catch {
    return DEFAULT_AUTH;
  }
}

export function saveAuth(auth: AuthState): void {
  localStorage.setItem(
    AUTH_KEY,
    JSON.stringify({
      authenticated: auth.authenticated
    })
  );
}

export function loadSessions(): ChatSession[] {
  try {
    const raw = localStorage.getItem(SESSIONS_KEY);
    if (!raw) {
      return [];
    }
    return JSON.parse(raw) as ChatSession[];
  } catch {
    return [];
  }
}

export function saveSessions(sessions: ChatSession[]): void {
  localStorage.setItem(SESSIONS_KEY, JSON.stringify(sessions));
}
