import { describe, expect, it } from "vitest";
import { buildChatSessionFromGateway, mapGatewayRoleToMessageRole, mergeHydratedSessionFromTimeline } from "./useChatApp";

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
    expect(session.isPlaceholderTitle).toBe(false);
  });

  it("prefers persisted remote title over timeline-derived title", () => {
    const session = buildChatSessionFromGateway(
      {
        session_id: "sess_2",
        title: "远端标题",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:05Z"
      },
      [
        {
          request_id: "req",
          session_id: "sess_2",
          event_seq: 1,
          timestamp: "2026-01-01T00:00:01Z",
          stream_source: "synthetic",
          type: "user_message",
          payload: { content: "这条不应覆盖远端标题" }
        }
      ]
    );
    expect(session.title).toBe("远端标题");
    expect(session.isPlaceholderTitle).toBe(false);
  });

  it("keeps placeholder flag when no remote title and no timeline user message", () => {
    const session = buildChatSessionFromGateway(
      {
        session_id: "sess_3",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:05Z"
      },
      []
    );
    expect(session.title).toBe("新聊天");
    expect(session.isPlaceholderTitle).toBe(true);
  });

  it("repairs placeholder title even when timeline has no incremental events", () => {
    const current = {
      id: "sess_4",
      title: "新聊天",
      isPlaceholderTitle: true,
      createdAt: "2026-01-01T00:00:00Z",
      connection: "idle" as const,
      turnPhase: "completed" as const,
      lastEventSeq: 3,
      messages: [
        {
          id: "m1",
          kind: "text" as const,
          role: "user" as const,
          content: "恢复成真实标题",
          timestamp: "2026-01-01T00:00:01Z",
          startEventSeq: 1,
          lastEventSeq: 1
        }
      ],
      pendingApprovals: []
    };
    const merged = mergeHydratedSessionFromTimeline(current, [
      {
        request_id: "req",
        session_id: "sess_4",
        event_seq: 3,
        timestamp: "2026-01-01T00:00:03Z",
        stream_source: "synthetic",
        type: "turn_completed",
        payload: {}
      }
    ]);
    expect(merged.title).toContain("恢复成真实标题");
    expect(merged.isPlaceholderTitle).toBe(false);
  });

  it("keeps other session title state isolated when hydrating one session", () => {
    const sessionA = {
      id: "sess_a",
      title: "新聊天",
      isPlaceholderTitle: true,
      createdAt: "2026-01-01T00:00:00Z",
      connection: "idle" as const,
      turnPhase: "completed" as const,
      lastEventSeq: 0,
      messages: [],
      pendingApprovals: []
    };
    const sessionB = {
      id: "sess_b",
      title: "B标题",
      isPlaceholderTitle: false,
      createdAt: "2026-01-01T00:00:00Z",
      connection: "idle" as const,
      turnPhase: "completed" as const,
      lastEventSeq: 2,
      messages: [],
      pendingApprovals: []
    };
    const mergedA = mergeHydratedSessionFromTimeline(sessionA, [
      {
        request_id: "req",
        session_id: "sess_a",
        event_seq: 1,
        timestamp: "2026-01-01T00:00:01Z",
        stream_source: "synthetic",
        type: "user_message",
        payload: { content: "A的新标题" }
      }
    ]);
    expect(mergedA.title).toContain("A的新标题");
    expect(mergedA.isPlaceholderTitle).toBe(false);
    expect(sessionB.title).toBe("B标题");
    expect(sessionB.isPlaceholderTitle).toBe(false);
  });
});
