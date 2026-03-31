import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../types/chat";
import { applySessionEvent } from "./reducer";

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

describe("session-events/tools", () => {
  it("upserts tool events with the same tool_call_id into one tool card", () => {
    const session = baseSession();
    const started = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell", display_name: "Run Shell", target: "pwd" }
    });
    const completed = applySessionEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: {
        tool_call_id: "call_1",
        tool_name: "shell",
        display_name: "Run Shell",
        ok: true,
        output: "stdout:\nok\nstderr:\n",
        shell_metadata: {
          result_class: "partial_success",
          backend: "macos_seatbelt",
          exit_code: 141,
          policy_decision: "allow",
          runtime_allowed: true,
          degrade_reason: null,
          runtime_deny_reason: null
        }
      }
    });

    const startedStep = started.messages.find((message) => message.kind === "tool_steps")?.toolSteps?.[0];
    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    const legacyToolMessages = completed.messages.filter(
      (message) => message.kind === "text" && message.role === "tool"
    );
    expect(stepMessages).toHaveLength(1);
    expect(legacyToolMessages).toHaveLength(0);
    expect(startedStep?.status).toBe("running");
    expect(stepMessages[0].toolSteps?.[0].status).toBe("success");
    expect(stepMessages[0].toolSteps?.[0].title).toBe("Run Shell");
    expect(stepMessages[0].toolSteps?.[0].description).toContain("Partial success");
    expect(stepMessages[0].toolSteps?.[0].meta?.backendSummary).toBe(
      "sandbox: sandbox-exec (macos_seatbelt)"
    );
    expect(stepMessages[0].toolSteps?.[0].durationSec).toBe(1);
    expect(stepMessages[0].toolSteps?.[0].startEventSeq).toBe(1);
    expect(stepMessages[0].toolSteps?.[0].lastEventSeq).toBe(2);
    expect(stepMessages[0].toolSteps?.[0].endEventSeq).toBe(2);
  });

  it("keeps different tool_call_id entries as separate cards even in same turn", () => {
    const session = baseSession();
    const firstStarted = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "shell", target: "pwd" }
    });

    const secondStarted = applySessionEvent(firstStarted, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_2", tool_name: "Read", target: "README.md" }
    });

    const stepMessages = secondStarted.messages.filter((message) => message.kind === "tool_steps");
    expect(stepMessages).toHaveLength(2);
    expect(stepMessages[0].toolSteps?.[0].toolCallId).toBe("call_1");
    expect(stepMessages[1].toolSteps?.[0].toolCallId).toBe("call_2");
  });

  it("does not merge when started/completed have different tool_call_id", () => {
    const session = baseSession();
    const started = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_call_id: "call_1", tool_name: "Read", target: "a.txt" }
    });
    const completed = applySessionEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_call_id: "call_2", tool_name: "Edit", output: "ok" }
    });

    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    expect(stepMessages).toHaveLength(2);
    expect(stepMessages[0]?.toolSteps?.[0].title).toBe("Read");
    expect(stepMessages[1]?.toolSteps?.[0].title).toBe("Edit");
  });

  it("falls back to synthetic ids when tool_call_id is missing", () => {
    const session = baseSession();
    const next = applySessionEvent(session, {
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

  it("tracks approvals lifecycle by updating the same card", () => {
    const session = baseSession();
    const requested = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "approval_requested",
      payload: { approval_id: "approval_1", tool_name: "shell", tool_call_id: "call_1" }
    });

    const resolved = applySessionEvent(requested, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "approval_resolved",
      payload: { approval_id: "approval_1", approved: true, tool_call_id: "call_1" }
    });

    const steps = resolved.messages
      .filter((message) => message.kind === "tool_steps")
      .map((message) => message.toolSteps?.[0])
      .filter(Boolean);
    expect(steps).toHaveLength(1);
    expect(steps[0]?.status).toBe("success");
    expect(steps[0]?.durationSec).toBe(1);
    expect(steps[0]?.startEventSeq).toBe(1);
    expect(steps[0]?.lastEventSeq).toBe(2);
    expect(steps[0]?.endEventSeq).toBe(2);
    expect(requested.pendingApprovals).toHaveLength(1);
    expect(requested.pendingApprovals[0]?.toolCallId).toBe("call_1");
    expect(resolved.pendingApprovals).toHaveLength(0);
  });

  it("falls back to approval_id merge when approval events do not include tool_call_id", () => {
    const session = baseSession();
    const requested = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "approval_requested",
      payload: { approval_id: "approval_2", tool_name: "shell" }
    });
    const resolved = applySessionEvent(requested, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "approval_resolved",
      payload: { approval_id: "approval_2", approved: true }
    });

    const steps = resolved.messages
      .filter((message) => message.kind === "tool_steps")
      .map((message) => message.toolSteps?.[0])
      .filter(Boolean);
    expect(steps).toHaveLength(1);
    expect(steps[0]?.approvalId).toBe("approval_2");
    expect(steps[0]?.status).toBe("success");
  });

  it("does not merge tool events when tool_call_id is missing", () => {
    const session = baseSession();
    const started = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_name: "shell", target: "pwd" }
    });
    const completed = applySessionEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_name: "shell", output: "ok" }
    });

    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    expect(stepMessages).toHaveLength(2);
  });

  it("does not merge missing-tool_call_id entries even when tool names match", () => {
    const session = baseSession();
    const started = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_started",
      payload: { tool_name: "Read", target: "a.txt" }
    });
    const completed = applySessionEvent(started, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_completed",
      payload: { tool_name: "Read", output: "ok" }
    });

    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    expect(stepMessages).toHaveLength(2);
  });

  it("emits failed summary tool_steps message on error and keeps error text", () => {
    const session = baseSession();
    const next = applySessionEvent(session, {
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

  it("does not render tool batch proposed/completed as standalone cards", () => {
    const session = baseSession();
    const proposed = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_9",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_calls_proposed",
      payload: { tool_calls: [{ tool_call_id: "call_1" }] }
    });
    const completed = applySessionEvent(proposed, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_9",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_batch_completed",
      payload: { total: 1, succeeded: 1, failed: 0 }
    });
    const stepMessages = completed.messages.filter((message) => message.kind === "tool_steps");
    expect(stepMessages).toHaveLength(0);
  });

  it("derives degraded shell warnings from shell_metadata", () => {
    const session = baseSession();
    const next = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "tool_call_completed",
      payload: {
        tool_call_id: "call_2",
        tool_name: "shell",
        ok: true,
        output: "command=git add -A && git commit -m \"x\"\nstdout:\nok\nstderr:\n",
        shell_metadata: {
          result_class: "success",
          backend: "none_escalated",
          exit_code: 0,
          policy_decision: "AskApproval",
          runtime_allowed: true,
          degrade_reason: "macos_seatbelt: denied",
          runtime_deny_reason: "skill_trigger_not_shell_command"
        }
      }
    });

    const step = next.messages.find((message) => message.kind === "tool_steps")?.toolSteps?.[0];
    expect(step?.status).toBe("success");
    expect(step?.meta?.backendSummary).toBe("sandbox: none (degraded)");
    expect(step?.meta?.riskSummary).toBe("risk: mutating command ran unsandboxed");
    expect(step?.meta?.hint).toBe(
      "hint: detected skill trigger string in shell; use skill workflow steps"
    );
  });
});
