import { beforeEach, describe, expect, it } from "vitest";
import { loadAuth, loadSettings, saveAuth, saveSettings } from "./storage";

describe("settings storage", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns defaults when empty", () => {
    const settings = loadSettings();
    expect(settings.baseUrl).toContain("127.0.0.1");
    expect(settings.outputMode).toBe("sse");
  });

  it("persists settings", () => {
    saveSettings({
      baseUrl: "http://localhost:8080",
      outputMode: "polling"
    });

    const settings = loadSettings();
    expect(settings.outputMode).toBe("polling");
    expect(settings.baseUrl).toBe("http://127.0.0.1:8080");
  });

  it("normalizes localhost gateway address", () => {
    saveSettings({
      baseUrl: "http://localhost:8765",
      outputMode: "sse"
    });
    expect(loadSettings().baseUrl).toBe("http://127.0.0.1:8765");
  });
});

describe("auth storage", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns defaults when empty", () => {
    const auth = loadAuth();
    expect(auth.authenticated).toBe(false);
    expect(auth.accessToken).toBe("");
  });

  it("persists auth state", () => {
    saveAuth({
      authenticated: true,
      accessToken: "atk_test",
      sessionId: "authsess_1",
      scope: "owner"
    });

    const auth = loadAuth();
    expect(auth.authenticated).toBe(true);
    expect(auth.accessToken).toBe("");
    expect(auth.sessionId).toBeNull();
  });
});
