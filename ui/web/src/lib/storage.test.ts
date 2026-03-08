import { beforeEach, describe, expect, it } from "vitest";
import { loadSettings, saveSettings } from "./storage";

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
      apiKey: "abc",
      baseUrl: "http://localhost:8080",
      outputMode: "polling"
    });

    const settings = loadSettings();
    expect(settings.apiKey).toBe("abc");
    expect(settings.outputMode).toBe("polling");
  });
});
