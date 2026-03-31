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

  it("splits reasoning blocks when reasoning_segment_id changes", () => {
    const session = baseSession();
    const first = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r2",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "reasoning_delta",
      payload: { content_delta: "思考A", reasoning_segment_id: "reason_1" }
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
      payload: { content_delta: "思考B", reasoning_segment_id: "reason_2" }
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

  it("keeps reasoning open across tool lifecycle events", () => {
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
    expect(assistant?.reasoningBlocks).toHaveLength(1);
    expect(assistant?.reasoningBlocks?.[0]?.content).toBe("调用前思考调用后思考");
    expect(assistant?.reasoningBlocks?.[0]?.closed).toBe(false);
    expect(assistant?.reasoningBlocks?.[0]?.startEventSeq).toBe(1);
    expect(assistant?.reasoningBlocks?.[0]?.lastEventSeq).toBe(3);
  });

  it("keeps one reasoning block when reasoning_segment_id stays the same across tool events", () => {
    const session = baseSession();
    const beforeTool = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3b",
      event_seq: 1,
      timestamp: "2026-01-01T00:00:01Z",
      type: "reasoning_delta",
      payload: { content_delta: "调用前思考", reasoning_segment_id: "reason_1" }
    });
    const toolEvent = applySessionEvent(beforeTool, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3b",
      event_seq: 2,
      timestamp: "2026-01-01T00:00:02Z",
      type: "tool_call_started",
      payload: { tool_calls: [{ tool_call_id: "call_x" }] }
    });
    const afterTool = applySessionEvent(toolEvent, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r3b",
      event_seq: 3,
      timestamp: "2026-01-01T00:00:03Z",
      type: "reasoning_delta",
      payload: { content_delta: "调用后思考", reasoning_segment_id: "reason_1" }
    });
    const assistant = afterTool.messages.find((message) => message.turnId === "turn_r3b" && message.role === "assistant");
    expect(assistant?.reasoningBlocks).toHaveLength(1);
    expect(assistant?.reasoningBlocks?.[0]?.content).toBe("调用前思考调用后思考");
    expect(assistant?.reasoningBlocks?.[0]?.blockId).toBe("reasoning:turn_r3b:reason_1");
  });

  it("tracks text seq independently from message start seq", () => {
    const session = baseSession();
    const reasoning = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r4",
      event_seq: 10,
      timestamp: "2026-01-01T00:00:10Z",
      type: "reasoning_delta",
      payload: { content_delta: "思考1" }
    });
    const tool = applySessionEvent(reasoning, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r4",
      event_seq: 11,
      timestamp: "2026-01-01T00:00:11Z",
      type: "tool_call_started",
      payload: { tool_name: "Read", tool_call_id: "call_1" }
    });
    const text = applySessionEvent(tool, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r4",
      event_seq: 13,
      timestamp: "2026-01-01T00:00:13Z",
      type: "response_text_delta",
      payload: { content_delta: "正文" }
    });
    const assistant = text.messages.find((message) => message.turnId === "turn_r4" && message.role === "assistant");
    expect(assistant?.startEventSeq).toBe(10);
    expect(assistant?.textStartEventSeq).toBe(13);
    expect(assistant?.textLastEventSeq).toBe(13);
  });

  it("backs text seq with response_completed when no text delta exists", () => {
    const session = baseSession();
    const completed = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r5",
      event_seq: 20,
      timestamp: "2026-01-01T00:00:20Z",
      type: "response_completed",
      payload: { content: "最终正文" }
    });
    const assistant = completed.messages.find((message) => message.turnId === "turn_r5" && message.role === "assistant");
    expect(assistant?.textStartEventSeq).toBe(20);
    expect(assistant?.textLastEventSeq).toBe(20);
    expect(assistant?.textEndEventSeq).toBe(20);
  });

  it("does not regress text seq on late assistant_message", () => {
    const session = baseSession();
    const textDelta = applySessionEvent(session, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r6",
      event_seq: 30,
      timestamp: "2026-01-01T00:00:30Z",
      type: "response_text_delta",
      payload: { content_delta: "正文1" }
    });
    const completed = applySessionEvent(textDelta, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r6",
      event_seq: 31,
      timestamp: "2026-01-01T00:00:31Z",
      type: "response_completed",
      payload: { content: "正文1" }
    });
    const lateAssistant = applySessionEvent(completed, {
      request_id: "req",
      session_id: "sess_1",
      turn_id: "turn_r6",
      event_seq: 32,
      timestamp: "2026-01-01T00:00:32Z",
      type: "assistant_message",
      payload: { content: "正" }
    });
    const assistant = lateAssistant.messages.find((message) => message.turnId === "turn_r6" && message.role === "assistant");
    expect(assistant?.content).toBe("正文1");
    expect(assistant?.textStartEventSeq).toBe(30);
    expect(assistant?.textLastEventSeq).toBe(32);
    expect(assistant?.textEndEventSeq).toBe(32);
  });
});
