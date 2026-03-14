import { memo, useEffect, useMemo, useRef } from "react";
import { useStreamSnapshot } from "../hooks/useStreamSnapshot";
import { recordAssistantPaneRender } from "../lib/devPerf";
import type { AssistantMessage, UserMessage } from "../types/chat";

interface ConversationPaneProps {
  sessionId?: string;
  turnId?: string;
  users: UserMessage[];
  assistants: AssistantMessage[];
}

type MessageItem =
  | { id: string; role: "user"; content: string; timestamp: string }
  | { id: string; role: "assistant"; content: string; timestamp: string };

function ConversationPane({ sessionId, turnId, users, assistants }: ConversationPaneProps) {
  const snapshot = useStreamSnapshot(sessionId, turnId);
  recordAssistantPaneRender(sessionId);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);

  const timeline = useMemo<MessageItem[]>(() => {
    const mixed: MessageItem[] = [
      ...users.map((item) => ({
        id: item.id,
        role: "user" as const,
        content: item.content,
        timestamp: item.timestamp
      })),
      ...assistants.map((item) => ({
        id: item.id,
        role: "assistant" as const,
        content: item.content,
        timestamp: item.timestamp
      }))
    ];

    mixed.sort((a, b) => {
      const delta = Date.parse(a.timestamp) - Date.parse(b.timestamp);
      if (delta !== 0) {
        return delta;
      }
      return a.id.localeCompare(b.id);
    });
    return mixed;
  }, [assistants, users]);

  useEffect(() => {
    const container = scrollRef.current;
    if (!container || !stickToBottomRef.current) {
      return;
    }
    container.scrollTop = container.scrollHeight;
  }, [timeline, snapshot.version]);

  return (
    <section className="panel conversation-panel">
      <h2>Conversation</h2>
      <div
        ref={scrollRef}
        className="conversation-scroll"
        onScroll={(event) => {
          const container = event.currentTarget;
          const distance = container.scrollHeight - (container.scrollTop + container.clientHeight);
          stickToBottomRef.current = distance <= 48;
        }}
      >
        {timeline.length === 0 ? <div className="empty">连接后开始对话</div> : null}

        {timeline.map((item) =>
          item.role === "user" ? (
            <div key={item.id} className="conversation-row user">
              <div className="user-bubble">{item.content}</div>
            </div>
          ) : (
            <div key={item.id} className="conversation-row assistant">
              <div className="assistant-text">{item.content}</div>
            </div>
          )
        )}

        {snapshot.version > 0 && snapshot.isActive ? (
          <div className="conversation-row assistant live-row">
            <div className="assistant-live-tag">Streaming</div>
            <div className="assistant-text assistant-live-inline">{snapshot.content}</div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

export default memo(ConversationPane);
