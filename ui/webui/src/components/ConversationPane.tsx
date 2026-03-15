import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useStreamSnapshot } from "../hooks/useStreamSnapshot";
import { recordAssistantPaneRender } from "../lib/devPerf";
import type { AssistantMessage, UserMessage } from "../types/chat";

interface ConversationPaneProps {
  sessionId?: string;
  turnId?: string;
  users: UserMessage[];
  assistants: AssistantMessage[];
  reasoningByTurn: Record<string, string>;
}

type MessageItem =
  | { id: string; role: "user"; content: string; timestamp: string }
  | { id: string; role: "assistant"; content: string; timestamp: string; turnId?: string };

function ConversationPane({ sessionId, turnId, users, assistants, reasoningByTurn }: ConversationPaneProps) {
  const snapshot = useStreamSnapshot(sessionId, turnId);
  recordAssistantPaneRender(sessionId);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);
  const prevLiveTextRef = useRef("");
  const [animatedDelta, setAnimatedDelta] = useState<{ stable: string; delta: string; key: number }>({
    stable: "",
    delta: "",
    key: 0
  });

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
        timestamp: item.timestamp,
        turnId: item.turnId
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
    const rafId = requestAnimationFrame(() => {
      container.scrollTop = container.scrollHeight;
    });
    return () => cancelAnimationFrame(rafId);
  }, [timeline.length, snapshot.version, snapshot.content, snapshot.isActive]);

  useEffect(() => {
    if (!snapshot.isActive) {
      prevLiveTextRef.current = snapshot.content;
      setAnimatedDelta((prev) => ({
        stable: snapshot.content,
        delta: "",
        key: prev.key
      }));
      return;
    }

    const prevText = prevLiveTextRef.current;
    const nextText = snapshot.content;
    if (!nextText.startsWith(prevText)) {
      prevLiveTextRef.current = nextText;
      setAnimatedDelta((prev) => ({
        stable: nextText,
        delta: "",
        key: prev.key
      }));
      return;
    }

    const delta = nextText.slice(prevText.length);
    if (!delta.length) {
      return;
    }

    prevLiveTextRef.current = nextText;
    setAnimatedDelta((prev) => ({
      stable: prevText,
      delta,
      key: prev.key + 1
    }));
  }, [snapshot.content, snapshot.isActive, snapshot.version]);

  return (
    <section className="panel conversation-panel">
      <h2>Conversation</h2>
      <div
        ref={scrollRef}
        className="conversation-scroll"
        onScroll={(event) => {
          const container = event.currentTarget;
          const distance = container.scrollHeight - (container.scrollTop + container.clientHeight);
          stickToBottomRef.current = distance <= 96;
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
              <div className="assistant-text-wrap">
                <div className="assistant-text">{item.content}</div>
                {item.turnId && reasoningByTurn[item.turnId] ? (
                  <details className="reasoning-panel">
                    <summary>思考过程</summary>
                    <pre className="reasoning-text">{reasoningByTurn[item.turnId]}</pre>
                  </details>
                ) : null}
              </div>
            </div>
          )
        )}

        {snapshot.version > 0 && snapshot.isActive ? (
          <div className="conversation-row assistant live-row">
            <div className="assistant-live-tag">Streaming</div>
            <div className="assistant-text-wrap">
              <div className="assistant-text assistant-live-inline">
                {animatedDelta.stable}
                {animatedDelta.delta ? (
                  <span key={animatedDelta.key} className="stream-reveal">
                    {animatedDelta.delta}
                  </span>
                ) : null}
              </div>
              {turnId && reasoningByTurn[turnId] ? (
                <details className="reasoning-panel">
                  <summary>思考过程</summary>
                  <pre className="reasoning-text">{reasoningByTurn[turnId]}</pre>
                </details>
              ) : null}
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

export default memo(ConversationPane);
