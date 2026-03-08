import type { AppSettings } from "../types/gateway";
import type { ChatSession } from "../types/chat";

const SETTINGS_KEY = "openjax:web:settings";
const SESSIONS_KEY = "openjax:web:sessions";

const DEFAULT_SETTINGS: AppSettings = {
  apiKey: "",
  baseUrl: "http://127.0.0.1:8765",
  outputMode: "sse"
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
      apiKey: parsed.apiKey ?? "",
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
