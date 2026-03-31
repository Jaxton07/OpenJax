import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../../types/chat";
import { applySessionEvent } from "../reducer";

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

describe("session-events assistant_message compat", () => {
  it("assistant_message alone does not finalize the turn", () => {
    const next = applySessionEvent(baseSession(), {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_legacy",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "assistant_message",
      payload: { content: "legacy final text" }
    });

    const assistant = next.messages.find(
      (message) => message.turnId === "turn_legacy" && message.role === "assistant"
    );

    expect(next.turnPhase).toBe("draft");
    expect(assistant?.content).toBe("legacy final text");
    expect(assistant?.isDraft).toBe(true);
  });
});
