import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../types/chat";
import { applySessionEvent, applySessionEvents } from "./reducer";

function baseSession(): ChatSession {
  return {
    id: "sess_1",
    title: "test",
    createdAt: "2026-01-01T00:00:00Z",
    connection: "active",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: []
  };
}

describe("session-events/reducer", () => {
  it("deduplicates event_seq", () => {
    const session = baseSession();
    const event = {
      request_id: "req_1",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 3,
      timestamp: "2026-01-01T00:00:01Z",
      type: "assistant_message",
      payload: { content: "hello" }
    } as const;

    const first = applySessionEvent(session, event);
    const second = applySessionEvent(first, event);

    expect(first.messages).toHaveLength(1);
    expect(second.messages).toHaveLength(1);
  });

  it("merges response text deltas into text messages only", () => {
    const session = baseSession();
    const delta1 = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "response_text_delta",
      payload: { content_delta: "hel" }
    });
    const delta2 = applySessionEvent(delta1, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "response_text_delta",
      payload: { content_delta: "lo" }
    });

    expect(delta2.messages).toHaveLength(1);
    expect(delta2.messages[0].kind).toBe("text");
    expect(delta2.messages[0].content).toBe("hello");
    expect(delta2.messages[0].startEventSeq).toBe(1);
    expect(delta2.messages[0].lastEventSeq).toBe(2);
  });

  it("supports v2 response_text_delta and response_completed aliases", () => {
    const session = baseSession();
    const delta = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_2",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "response_text_delta",
      payload: { content_delta: "hi", stream_source: "model_live" }
    });
    const done = applySessionEvent(delta, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_2",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "response_completed",
      payload: { content: "hi there" }
    });
    expect(done.turnPhase).toBe("completed");
    expect(done.messages.find((message) => message.turnId === "turn_2")?.content).toBe("hi");
  });

  it("applies batched events and keeps seq monotonic", () => {
    const session = baseSession();
    const next = applySessionEvents(session, [
      {
        request_id: "req",
        session_id: "sess_1",
        turn_id: "turn_3",
        event_seq: 1,
        timestamp: "2026-01-01T00:00:01Z",
        type: "response_text_delta",
        payload: { content_delta: "a", stream_source: "model_live" }
      },
      {
        request_id: "req",
        session_id: "sess_1",
        turn_id: "turn_3",
        event_seq: 2,
        timestamp: "2026-01-01T00:00:02Z",
        type: "response_text_delta",
        payload: { content_delta: "b", stream_source: "model_live" }
      }
    ]);
    expect(next.lastEventSeq).toBe(2);
    expect(next.messages.find((message) => message.turnId === "turn_3")?.content).toBe("ab");
  });

  it("accepts sequence reset when server event_seq restarts at 1", () => {
    const session: ChatSession = { ...baseSession(), lastEventSeq: 999 };
    const next = applySessionEvents(session, [
      {
        request_id: "req",
        session_id: "sess_1",
        turn_id: "turn_4",
        event_seq: 1,
        turn_seq: 1,
        timestamp: "2026-01-01T00:00:01Z",
        type: "response_text_delta",
        payload: { content_delta: "你", stream_source: "model_live" }
      },
      {
        request_id: "req",
        session_id: "sess_1",
        turn_id: "turn_4",
        event_seq: 2,
        turn_seq: 2,
        timestamp: "2026-01-01T00:00:02Z",
        type: "response_completed",
        payload: { content: "你好" }
      }
    ]);
    expect(next.lastEventSeq).toBe(2);
    expect(next.messages.find((message) => message.turnId === "turn_4")?.content).toBe("你");
  });

  it("drops mapped legacy response_text_delta when canonical v2 delta already exists", () => {
    const session = baseSession();
    const first = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_dup",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "response_text_delta",
      payload: { content_delta: "A", stream_source: "model_live" }
    });
    const second = applySessionEvent(first, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_dup",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:01Z",
      type: "response_text_delta",
      payload: { content_delta: "A" }
    });

    expect(second.messages.find((message) => message.turnId === "turn_dup")?.content).toBe("A");
  });

  it("assistant_message does not overwrite existing draft content", () => {
    const session = baseSession();
    const draft = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_fallback",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "response_text_delta",
      payload: { content_delta: "draft", stream_source: "model_live" }
    });
    const assistantMessage = applySessionEvent(draft, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_fallback",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "assistant_message",
      payload: { content: "final text from message" }
    });

    expect(assistantMessage.messages.find((message) => message.turnId === "turn_fallback")?.content).toBe("draft");
  });

  it("marks connection closed on session_shutdown", () => {
    const session = { ...baseSession(), turnPhase: "streaming" as const };
    const next = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "session_shutdown",
      payload: {}
    });
    expect(next.connection).toBe("closed");
    expect(next.turnPhase).toBe("completed");
  });

  it("writes seq metadata when error message is emitted", () => {
    const session = baseSession();
    const next = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_err",
      event_seq: 7,
      timestamp: "2026-01-01T00:00:07Z",
      type: "response_error",
      payload: { message: "boom" }
    });
    const error = next.messages.find((message) => message.role === "error");
    expect(error?.startEventSeq).toBe(7);
    expect(error?.lastEventSeq).toBe(7);
  });
});
