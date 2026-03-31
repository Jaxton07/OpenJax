import { describe, expect, it } from "vitest";
import { applyTextStreamEvent, isTextStreamEvent } from "./streamRuntime";
import type { ChatSession } from "../types/chat";
import type { StreamEvent } from "../types/gateway";

function baseSession(): ChatSession {
  return {
    id: "sess_1",
    title: "test",
    isPlaceholderTitle: false,
    createdAt: "2026-01-01T00:00:00Z",
    connection: "active",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: []
  };
}

function event(partial: Partial<StreamEvent> & Pick<StreamEvent, "type" | "event_seq">): StreamEvent {
  return {
    request_id: "req_1",
    session_id: "sess_1",
    turn_id: "turn_1",
    timestamp: "2026-01-01T00:00:00Z",
    payload: {},
    ...partial
  };
}

describe("streamRuntime", () => {
  it("streams text deltas incrementally for a turn", () => {
    let session = baseSession();
    session = applyTextStreamEvent(
      session,
      event({ type: "response_started", event_seq: 1, payload: {} })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 2, payload: { content_delta: "你" } })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 3, payload: { content_delta: "好" } })
    );

    expect(session.turnPhase).toBe("streaming");
    expect(session.streaming?.active).toBe(true);
    expect(session.streaming?.content).toBe("你好");
    expect(session.messages.find((message) => message.turnId === "turn_1")?.content).toBe("你好");
  });

  it("deduplicates replayed events by event_seq gate", () => {
    let session = baseSession();
    session = applyTextStreamEvent(
      session,
      event({ type: "response_started", event_seq: 1, payload: {} })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 2, payload: { content_delta: "A" } })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 2, payload: { content_delta: "A" } })
    );

    expect(session.messages.find((message) => message.turnId === "turn_1")?.content).toBe("A");
    expect(session.lastEventSeq).toBe(2);
  });

  it("finalizes draft on response_completed without overwriting streamed content", () => {
    let session = baseSession();
    session = applyTextStreamEvent(
      session,
      event({ type: "response_started", event_seq: 1, payload: {} })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 2, payload: { content_delta: "你好" } })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_completed", event_seq: 3, payload: { content: "你好！有什么我可以帮您的吗？" } })
    );

    const assistant = session.messages.find((message) => message.turnId === "turn_1");
    expect(session.turnPhase).toBe("completed");
    expect(session.streaming?.active).toBe(false);
    expect(assistant?.isDraft).toBe(false);
    expect(assistant?.content).toBe("你好！有什么我可以帮您的吗？");
  });

  it("accepts only designated text stream event types", () => {
    expect(isTextStreamEvent(event({ type: "assistant_message", event_seq: 0 }))).toBe(false);
    expect(isTextStreamEvent(event({ type: "response_started", event_seq: 1 }))).toBe(true);
    expect(isTextStreamEvent(event({ type: "response_text_delta", event_seq: 2 }))).toBe(true);
    expect(isTextStreamEvent(event({ type: "response_completed", event_seq: 3 }))).toBe(true);
    expect(isTextStreamEvent(event({ type: "response_error", event_seq: 4 }))).toBe(true);
    expect(isTextStreamEvent(event({ type: "tool_call_started", event_seq: 5 }))).toBe(false);
  });

  it("ignores assistant_message for completion semantics", () => {
    let session = baseSession();
    session = applyTextStreamEvent(
      session,
      event({ type: "response_started", event_seq: 1, payload: {} })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "response_text_delta", event_seq: 2, payload: { content_delta: "你有什" } })
    );
    session = applyTextStreamEvent(
      session,
      event({ type: "assistant_message", event_seq: 3, payload: { content: "你好！有什么我可以帮你的吗？" } })
    );

    const assistant = session.messages.find((message) => message.turnId === "turn_1");
    expect(session.turnPhase).toBe("streaming");
    expect(session.streaming?.active).toBe(true);
    expect(assistant?.isDraft).toBe(true);
    expect(assistant?.content).toBe("你有什");
  });
});
