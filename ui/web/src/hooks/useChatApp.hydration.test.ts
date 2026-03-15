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
          message_id: "m2",
          session_id: "sess_1",
          turn_id: "turn_1",
          role: "assistant",
          content: "你好",
          sequence: 2,
          created_at: "2026-01-01T00:00:02Z"
        },
        {
          message_id: "m1",
          session_id: "sess_1",
          turn_id: "turn_1",
          role: "user",
          content: "帮我看一下配置",
          sequence: 1,
          created_at: "2026-01-01T00:00:01Z"
        }
      ]
    );
    expect(session.id).toBe("sess_1");
    expect(session.messages[0].id).toBe("m1");
    expect(session.messages[1].id).toBe("m2");
    expect(session.title).toContain("帮我看一下配置");
  });
});
