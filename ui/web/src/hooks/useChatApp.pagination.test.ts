import { describe, expect, it, vi } from "vitest";
import { buildSidebarSessionsFromSummaries, fetchSidebarSessionSummaries } from "./useChatApp";

describe("useChatApp sidebar pagination", () => {
  it("loads session sidebar with cursor pagination only", async () => {
    const listChatSessions = vi
      .fn()
      .mockResolvedValueOnce({
        request_id: "req_1",
        sessions: [
          {
            session_id: "sess_a",
            title: "A",
            created_at: "2026-03-30T10:00:00.000Z",
            updated_at: "2026-03-30T10:00:00.000Z"
          }
        ],
        next_cursor: "cursor_1",
        timestamp: "2026-03-30T10:00:01.000Z"
      });

    const page = await fetchSidebarSessionSummaries({ listChatSessions }, undefined, 20);

    expect(listChatSessions).toHaveBeenCalledTimes(1);
    expect(listChatSessions).toHaveBeenCalledWith({ cursor: undefined, limit: 20 });
    expect(page.summaries.map((item) => item.session_id)).toEqual(["sess_a"]);
    expect(page.nextCursor).toBe("cursor_1");
  });

  it("builds sidebar sessions without timeline hydration", () => {
    const sessions = buildSidebarSessionsFromSummaries([
      {
        session_id: "sess_sidebar",
        title: "sidebar only",
        created_at: "2026-03-30T08:00:00.000Z",
        updated_at: "2026-03-30T08:00:10.000Z"
      }
    ]);

    expect(sessions).toHaveLength(1);
    expect(sessions[0].id).toBe("sess_sidebar");
    expect(sessions[0].messages).toEqual([]);
    expect(sessions[0].pendingApprovals).toEqual([]);
    expect(sessions[0].lastEventSeq).toBe(0);
  });
});
