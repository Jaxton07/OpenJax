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

  it("merges assistant deltas into text messages only", () => {
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

    expect(delta2.messages).toHaveLength(1);
    expect(delta2.messages[0].kind).toBe("text");
    expect(delta2.messages[0].content).toBe("hello");
  });

  it("creates a tool_steps message for each tool event and keeps legacy tool text messages", () => {
    const session = baseSession();
    const started = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell", target: "pwd" }
    });
    const completed = applyStreamEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_call_id: "call_1", tool_name: "shell", output: "ok" }
    });

    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    const legacyToolMessages = completed.messages.filter(
      (message) => message.kind === "text" && message.role === "tool"
    );
    expect(stepMessages).toHaveLength(2);
    expect(legacyToolMessages).toHaveLength(2);
    expect(stepMessages[0].toolSteps?.[0].status).toBe("running");
    expect(stepMessages[1].toolSteps?.[0].status).toBe("success");
  });

  it("falls back to synthetic ids when tool_call_id is missing", () => {
    const session = baseSession();
    const next = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_name: "shell" }
    });

    const step = next.messages.find((message) => message.kind === "tool_steps")?.toolSteps?.[0];
    expect(step?.id).toContain("tool_call_started:turn_1:1");
  });

  it("tracks approvals lifecycle and emits tool_steps entries", () => {
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

    const steps = resolved.messages
      .filter((message) => message.kind === "tool_steps")
      .map((message) => message.toolSteps?.[0])
      .filter(Boolean);
    expect(steps).toHaveLength(2);
    expect(steps[0]?.status).toBe("waiting");
    expect(steps[1]?.status).toBe("success");
    expect(requested.pendingApprovals).toHaveLength(1);
    expect(resolved.pendingApprovals).toHaveLength(0);
  });

  it("emits failed summary tool_steps message on error and keeps error text", () => {
    const session = baseSession();
    const next = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "error",
      payload: { message: "boom" }
    });

    const step = next.messages.find((message) => message.kind === "tool_steps")?.toolSteps?.[0];
    const err = next.messages.find((message) => message.kind === "text" && message.role === "error");
    expect(step?.type).toBe("summary");
    expect(step?.status).toBe("failed");
    expect(err?.content).toBe("boom");
  });

  it("handles incomplete payload without throwing", () => {
    const session = baseSession();
    const next = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: {}
    });

    const step = next.messages.find((message) => message.kind === "tool_steps")?.toolSteps?.[0];
    expect(step?.title).toBe("tool");
    expect(step?.status).toBe("running");
  });
});
