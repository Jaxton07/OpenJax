import { describe, expect, it } from "vitest";
import { parseSseData } from "./gatewayClient";

describe("parseSseData", () => {
  it("parses valid stream event envelope", () => {
    const raw = JSON.stringify({
      request_id: "req_1",
      session_id: "sess_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:00Z",
      type: "response_started",
      payload: {}
    });
    const parsed = parseSseData(raw);
    expect(parsed?.type).toBe("response_started");
    expect(parsed?.event_seq).toBe(1);
  });

  it("returns null for invalid JSON", () => {
    expect(parseSseData("not-json")).toBeNull();
  });
});
