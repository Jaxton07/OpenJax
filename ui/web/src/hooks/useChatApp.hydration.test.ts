import { describe, expect, it } from "vitest";
import { buildChatSessionFromGateway, mapGatewayRoleToMessageRole } from "./useChatApp";

describe("useChatApp hydration helpers", () => {
  it("maps unknown gateway role to system", () => {
    expect(mapGatewayRoleToMessageRole("unknown")).toBe("system");
    expect(mapGatewayRoleToMessageRole("assistant")).toBe("assistant");
  });

  it("builds chat session from gateway records", () => {
    const session = buildChatSessionFromGateway(
      {
        session_id: "sess_1",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:05Z"
      },
      [
        {
          request_id: "req",
          session_id: "sess_1",
          turn_id: "turn_1",
          event_seq: 2,
          turn_seq: 1,
          timestamp: "2026-01-01T00:00:02Z",
          stream_source: "synthetic",
          type: "response_completed",
          payload: { content: "你好" }
        },
        {
          request_id: "req",
          session_id: "sess_1",
          event_seq: 1,
          timestamp: "2026-01-01T00:00:01Z",
          stream_source: "synthetic",
          type: "user_message",
          payload: { content: "帮我看一下配置" }
        }
      ]
    );
    expect(session.id).toBe("sess_1");
    expect(session.messages[0].role).toBe("user");
    expect(session.messages[1].role).toBe("assistant");
    expect(session.title).toContain("帮我看一下配置");
  });
});
