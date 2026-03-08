import { describe, expect, it } from "vitest";
import { applyStreamEvent } from "./eventReducer";
import type { ChatSession } from "../types/chat";

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

describe("applyStreamEvent", () => {
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

    const first = applyStreamEvent(session, event);
    const second = applyStreamEvent(first, event);

    expect(first.messages).toHaveLength(1);
    expect(second.messages).toHaveLength(1);
  });

  it("merges assistant deltas", () => {
    const session = baseSession();
    const delta1 = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "assistant_delta",
      payload: { content_delta: "hel" }
    });
    const delta2 = applyStreamEvent(delta1, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "assistant_delta",
      payload: { content_delta: "lo" }
    });

    expect(delta2.messages[0].content).toBe("hello");
  });

  it("tracks approvals lifecycle", () => {
    const session = baseSession();
    const requested = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "approval_requested",
      payload: { approval_id: "approval_1", tool_name: "shell" }
    });

    const resolved = applyStreamEvent(requested, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "approval_resolved",
      payload: { approval_id: "approval_1", approved: true }
    });

    expect(requested.pendingApprovals).toHaveLength(1);
    expect(resolved.pendingApprovals).toHaveLength(0);
  });
});
