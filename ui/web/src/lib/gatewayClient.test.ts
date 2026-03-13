import { describe, expect, it } from "vitest";
import { splitSseBuffer } from "./gatewayClient";

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
});
