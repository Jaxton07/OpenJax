import { afterEach, describe, expect, it, vi } from "vitest";
import { GatewayClient, splitSseBuffer, splitSseBufferAtStreamEnd } from "./gatewayClient";

describe("splitSseBuffer", () => {
  it("splits LF-delimited SSE chunks", () => {
    const source = "event: response_started\ndata: {}\n\n";
    const parsed = splitSseBuffer(source);
    expect(parsed.chunks).toEqual(["event: response_started\ndata: {}"]);
    expect(parsed.remainder).toBe("");
  });

  it("splits CRLF-delimited SSE chunks", () => {
    const source = "event: response_started\r\ndata: {}\r\n\r\n";
    const parsed = splitSseBuffer(source);
    expect(parsed.chunks).toEqual(["event: response_started\ndata: {}"]);
    expect(parsed.remainder).toBe("");
  });

  it("keeps an incomplete tail as remainder", () => {
    const source = "event: response_started\ndata: {}\n\nevent: response_text_delta\ndata: {\"a\":1}";
    const parsed = splitSseBuffer(source);
    expect(parsed.chunks).toEqual(["event: response_started\ndata: {}"]);
    expect(parsed.remainder).toBe("event: response_text_delta\ndata: {\"a\":1}");
  });

  it("flushes incomplete tail chunk when stream ends", () => {
    const source =
      "event: response_started\ndata: {}\n\nevent: response_completed\ndata: {\"event_seq\":2}";
    const chunks = splitSseBufferAtStreamEnd(source);
    expect(chunks).toEqual([
      "event: response_started\ndata: {}",
      "event: response_completed\ndata: {\"event_seq\":2}"
    ]);
  });
});

describe("GatewayClient listChatSessions", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends cursor and limit query params", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        request_id: "req",
        sessions: [],
        timestamp: "2026-03-30T10:00:00.000Z"
      })
    });
    vi.stubGlobal("fetch", fetchMock);

    const client = new GatewayClient({
      baseUrl: "http://127.0.0.1:8080",
      accessToken: "token"
    });
    await client.listChatSessions({ cursor: "abc", limit: 25 });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const url = String(fetchMock.mock.calls[0][0]);
    expect(url).toContain("/api/v1/sessions?cursor=abc&limit=25");
  });
});
