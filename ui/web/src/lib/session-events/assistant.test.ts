import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../types/chat";
import { applySessionEvent } from "./reducer";

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

describe("session-events/assistant", () => {
  it("appends continuous reasoning_delta into one open block", () => {
    const session = baseSession();
    const first = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r1",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "reasoning_delta",
      payload: { content_delta: "先分析" }
    });
    const second = applySessionEvent(first, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r1",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "reasoning_delta",
      payload: { content_delta: "再推理" }
    });
    const assistant = second.messages.find((message) => message.turnId === "turn_r1" && message.role === "assistant");
    expect(assistant?.reasoningBlocks).toHaveLength(1);
    expect(assistant?.reasoningBlocks?.[0]?.content).toBe("先分析再推理");
    expect(assistant?.reasoningBlocks?.[0]?.collapsed).toBe(true);
    expect(assistant?.reasoningBlocks?.[0]?.closed).toBe(false);
    expect(assistant?.reasoningBlocks?.[0]?.startEventSeq).toBe(1);
    expect(assistant?.reasoningBlocks?.[0]?.lastEventSeq).toBe(2);
  });

  it("splits reasoning blocks when response_text_delta starts", () => {
    const session = baseSession();
    const first = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r2",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "reasoning_delta",
      payload: { content_delta: "思考A" }
    });
    const textStarted = applySessionEvent(first, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r2",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "response_text_delta",
      payload: { content_delta: "正文A" }
    });
    const secondReasoning = applySessionEvent(textStarted, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r2",
      event_seq: 3,
      timestamp: "2026-01-01T00:00:03Z",
      type: "reasoning_delta",
      payload: { content_delta: "思考B" }
    });
    const assistant = secondReasoning.messages.find((message) => message.turnId === "turn_r2" && message.role === "assistant");
    expect(assistant?.reasoningBlocks).toHaveLength(2);
    expect(assistant?.reasoningBlocks?.[0]?.closed).toBe(true);
    expect(assistant?.reasoningBlocks?.[0]?.endEventSeq).toBe(1);
    expect(assistant?.reasoningBlocks?.[1]?.content).toBe("思考B");
    expect(assistant?.reasoningBlocks?.[1]?.closed).toBe(false);
    expect(assistant?.reasoningBlocks?.[1]?.startEventSeq).toBe(3);
    expect(assistant?.reasoningBlocks?.[1]?.lastEventSeq).toBe(3);
  });

  it("splits reasoning blocks around tool lifecycle events", () => {
    const session = baseSession();
    const beforeTool = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "reasoning_delta",
      payload: { content_delta: "调用前思考" }
    });
    const toolEvent = applySessionEvent(beforeTool, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_calls_proposed",
      payload: { tool_calls: [{ tool_call_id: "call_x" }] }
    });
    const afterTool = applySessionEvent(toolEvent, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3",
      event_seq: 3,
      timestamp: "2026-01-01T00:00:03Z",
      type: "reasoning_delta",
      payload: { content_delta: "调用后思考" }
    });
    const assistant = afterTool.messages.find((message) => message.turnId === "turn_r3" && message.role === "assistant");
    expect(assistant?.reasoningBlocks).toHaveLength(2);
    expect(assistant?.reasoningBlocks?.[0]?.content).toBe("调用前思考");
    expect(assistant?.reasoningBlocks?.[0]?.closed).toBe(true);
    expect(assistant?.reasoningBlocks?.[0]?.endEventSeq).toBe(1);
    expect(assistant?.reasoningBlocks?.[1]?.content).toBe("调用后思考");
    expect(assistant?.reasoningBlocks?.[1]?.startEventSeq).toBe(3);
  });
});
