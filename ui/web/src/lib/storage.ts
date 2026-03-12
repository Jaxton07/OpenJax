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
  apiKey: "",
  authenticated: false
};

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
    const baseUrl =
      parsed.baseUrl === "http://127.0.0.1:8080"
        ? DEFAULT_SETTINGS.baseUrl
        : parsed.baseUrl;
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
    if (!parsed.apiKey || !parsed.authenticated) {
      return DEFAULT_AUTH;
    }
    return {
      apiKey: parsed.apiKey,
      authenticated: parsed.authenticated
    };
  } catch {
    return DEFAULT_AUTH;
  }
}

export function saveAuth(auth: AuthState): void {
  localStorage.setItem(AUTH_KEY, JSON.stringify(auth));
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
