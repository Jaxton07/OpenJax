import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../types/chat";
import { deriveComposerPhase } from "../deriveComposerPhase";

function baseSession(overrides: Partial<ChatSession> = {}): ChatSession {
  return {
    id: "sess_1",
    title: "test",
    isPlaceholderTitle: false,
    createdAt: "2026-01-01T00:00:00Z",
    connection: "active",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: [],
    ...overrides
  };
}

describe("deriveComposerPhase", () => {
  it("returns idle when session is null", () => {
    expect(deriveComposerPhase(null)).toBe("idle");
  });

  it("returns idle when turnPhase is draft", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "draft" }))).toBe("idle");
  });

  it("returns idle when turnPhase is completed", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "completed" }))).toBe("idle");
  });

  it("returns working when turnPhase is submitting", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "submitting" }))).toBe("working");
  });

  it("returns working when streaming with no draft message", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "streaming" }))).toBe("working");
  });

  it("returns thinking when streaming draft has an open reasoning block", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          reasoningBlocks: [
            {
              blockId: "rb_1",
              turnId: "turn_1",
              content: "hmm...",
              collapsed: false,
              startedAt: "2026-01-01T00:00:00Z",
              closed: false
            }
          ]
        }
      ]
    });
    expect(deriveComposerPhase(session)).toBe("thinking");
  });

  it("returns working when streaming draft has only closed reasoning blocks and no content", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          reasoningBlocks: [
            {
              blockId: "rb_1",
              turnId: "turn_1",
              content: "done thinking",
              collapsed: false,
              startedAt: "2026-01-01T00:00:00Z",
              closed: true
            }
          ]
        }
      ]
    });
    expect(deriveComposerPhase(session)).toBe("working");
  });

  it("returns idle when streaming draft has text content", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "Here is my answer...",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true
        }
      ]
    });
    expect(deriveComposerPhase(session)).toBe("idle");
  });

  it("returns working when streaming with running tool steps", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "tool_steps",
          role: "tool",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          toolSteps: [
            {
              id: "step_1",
              type: "tool",
              title: "bash",
              status: "running",
              time: "2026-01-01T00:00:00Z"
            }
          ]
        }
      ]
    });
    expect(deriveComposerPhase(session)).toBe("working");
  });
});
