import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../../types/chat";
import { buildTimeline } from "./buildTimeline";

describe("buildTimeline", () => {
  it("orders reasoning/tool/reasoning/assistant by event_seq", () => {
    const messages: ChatMessage[] = [
      {
        id: "assistant_1",
        kind: "text",
        role: "assistant",
        content: "最终正文",
        timestamp: "2026-01-01T00:00:03Z",
        turnId: "turn_1",
        startEventSeq: 10,
        lastEventSeq: 13,
        textStartEventSeq: 13,
        textLastEventSeq: 13,
        textEndEventSeq: 13,
        reasoningBlocks: [
          {
            blockId: "r1",
            turnId: "turn_1",
            content: "先想第一步",
            collapsed: true,
            startedAt: "2026-01-01T00:00:01Z",
            closed: true,
            startEventSeq: 10,
            lastEventSeq: 10,
            endEventSeq: 10
          },
          {
            blockId: "r2",
            turnId: "turn_1",
            content: "再想第二步",
            collapsed: true,
            startedAt: "2026-01-01T00:00:02Z",
            closed: true,
            startEventSeq: 12,
            lastEventSeq: 12,
            endEventSeq: 12
          }
        ]
      },
      {
        id: "tool_msg",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:02Z",
        turnId: "turn_1",
        startEventSeq: 11,
        lastEventSeq: 11,
        toolSteps: [
          {
            id: "step_1",
            type: "tool",
            title: "Read",
            status: "success",
            time: "2026-01-01T00:00:02Z",
            startEventSeq: 11,
            lastEventSeq: 11,
            endEventSeq: 11
          }
        ]
      }
    ];

    const timeline = buildTimeline(messages);
    expect(timeline.map((item) => item.type)).toEqual([
      "reasoning_block",
      "tool_step",
      "reasoning_block",
      "assistant_text"
    ]);
  });

  it("uses seq before timestamp when both exist", () => {
    const messages: ChatMessage[] = [
      {
        id: "a1",
        kind: "text",
        role: "assistant",
        content: "A",
        timestamp: "2026-01-01T00:00:10Z",
        startEventSeq: 2,
        lastEventSeq: 2
      },
      {
        id: "a2",
        kind: "text",
        role: "assistant",
        content: "B",
        timestamp: "2026-01-01T00:00:01Z",
        startEventSeq: 3,
        lastEventSeq: 3
      }
    ];

    const timeline = buildTimeline(messages);
    expect(timeline[0]?.payload.message.id).toBe("a1");
    expect(timeline[1]?.payload.message.id).toBe("a2");
  });

  it("uses text seq for assistant_text ordering before message start seq", () => {
    const messages: ChatMessage[] = [
      {
        id: "assistant_early_start",
        kind: "text",
        role: "assistant",
        content: "最终正文",
        timestamp: "2026-01-01T00:00:03Z",
        startEventSeq: 1,
        lastEventSeq: 8,
        textStartEventSeq: 8,
        textLastEventSeq: 8
      },
      {
        id: "tool_msg",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:02Z",
        startEventSeq: 5,
        lastEventSeq: 5,
        toolSteps: [
          {
            id: "step_1",
            type: "tool",
            title: "Read",
            status: "success",
            time: "2026-01-01T00:00:02Z",
            startEventSeq: 5,
            lastEventSeq: 5,
            endEventSeq: 5
          }
        ]
      }
    ];

    const timeline = buildTimeline(messages);
    expect(timeline.map((item) => item.type)).toEqual(["tool_step", "assistant_text"]);
  });

  it("keeps stable ordering for same seq and timestamp", () => {
    const messages: ChatMessage[] = [
      {
        id: "u1",
        kind: "text",
        role: "user",
        content: "Q",
        timestamp: "2026-01-01T00:00:01Z",
        startEventSeq: 10,
        lastEventSeq: 10
      },
      {
        id: "e1",
        kind: "text",
        role: "error",
        content: "E",
        timestamp: "2026-01-01T00:00:01Z",
        startEventSeq: 10,
        lastEventSeq: 10
      }
    ];

    const timeline = buildTimeline(messages);
    expect(timeline.map((item) => item.id)).toEqual(["user_text:u1", "error_text:e1"]);
  });
});
