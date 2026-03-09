import { describe, expect, it } from "vitest";
import { applyStreamEvent } from "./eventReducer";
import type { ChatSession, ToolStep } from "../types/chat";

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

  it("creates running tool step and keeps legacy tool message", () => {
    const session = baseSession();
    const next = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell", target: "zsh -lc \"ls\"" }
    });

    expect(next.messages.some((message) => message.role === "tool")).toBe(true);
    const steps = getToolSteps(next);
    expect(steps).toHaveLength(1);
    expect(steps[0]).toMatchObject({
      id: "call_1",
      type: "tool",
      title: "shell",
      status: "running"
    });
  });

  it("updates existing step from started to completed", () => {
    const session = baseSession();
    const started = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell" }
    });
    const completed = applyStreamEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_call_id: "call_1", tool_name: "shell", output: "done" }
    });

    const steps = getToolSteps(completed);
    expect(steps).toHaveLength(1);
    expect(steps[0].status).toBe("success");
    expect(steps[0].output).toBe("done");
  });

  it("falls back when tool_call_id is missing and still updates completion", () => {
    const session = baseSession();
    const started = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_name: "shell", target: "pwd" }
    });
    const completed = applyStreamEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_name: "shell", output: "ok" }
    });

    const steps = getToolSteps(completed);
    expect(steps).toHaveLength(1);
    expect(steps[0].status).toBe("success");
    expect(steps[0].output).toBe("ok");
  });

  it("writes approval step and resolves it", () => {
    const session = baseSession();
    const requested = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "approval_requested",
      payload: { approval_id: "approval_1", reason: "need write", target: "/tmp/a.txt" }
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

    const steps = getToolSteps(resolved);
    expect(steps.find((step) => step.id === "approval_1")?.status).toBe("success");
    expect(resolved.pendingApprovals).toHaveLength(0);
  });

  it("marks latest running step failed on error", () => {
    const session = baseSession();
    const started = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell" }
    });
    const errored = applyStreamEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "error",
      payload: { message: "boom" }
    });

    const steps = getToolSteps(errored);
    expect(steps[0].status).toBe("failed");
    expect(steps[0].output).toBe("boom");
  });

  it("creates failed summary step when no running step exists", () => {
    const session = baseSession();
    const errored = applyStreamEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:02Z",
      type: "error",
      payload: { message: "turn failed" }
    });

    const steps = getToolSteps(errored);
    expect(steps).toHaveLength(1);
    expect(steps[0].type).toBe("summary");
    expect(steps[0].status).toBe("failed");
  });

  it("ignores incomplete payload without throwing", () => {
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

    const steps = getToolSteps(next);
    expect(steps[0].title).toBe("tool");
    expect(steps[0].status).toBe("running");
  });
});

function getToolSteps(session: ChatSession): ToolStep[] {
  return session.messages.find((message) => Array.isArray(message.toolSteps))?.toolSteps ?? [];
}
