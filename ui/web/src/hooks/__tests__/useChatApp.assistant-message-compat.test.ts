import { describe, expect, it } from "vitest";
import { buildChatSessionFromGateway } from "../useChatApp";

describe("useChatApp assistant_message compat", () => {
  it("assistant_message alone does not finalize the hydrated session", () => {
    const session = buildChatSessionFromGateway(
      {
        session_id: "sess_1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:01Z"
      },
      [
        {
          request_id: "req",
          session_id: "sess_1",
          turn_id: "turn_legacy",
          event_seq: 1,
          timestamp: "2026-01-01T00:00:01Z",
          type: "assistant_message",
          payload: { content: "legacy final text" }
        }
      ]
    );

    const assistant = session.messages.find(
      (message) => message.turnId === "turn_legacy" && message.role === "assistant"
    );

    expect(session.turnPhase).toBe("draft");
    expect(assistant?.content).toBe("legacy final text");
    expect(assistant?.isDraft).toBe(true);
  });
});
