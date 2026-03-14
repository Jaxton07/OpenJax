import { memo, useMemo } from "react";
import { useStreamSnapshot } from "../hooks/useStreamSnapshot";
import { recordAssistantPaneRender } from "../lib/devPerf";
import type { AssistantMessage } from "../types/chat";

export interface AssistantStreamPaneProps {
  sessionId?: string;
  turnId?: string;
  fallbackContent?: string;
  history: AssistantMessage[];
}

function AssistantStreamPane({ sessionId, turnId, fallbackContent = "", history }: AssistantStreamPaneProps) {
  const snapshot = useStreamSnapshot(sessionId, turnId);
  recordAssistantPaneRender(sessionId);

  const liveText = useMemo(() => {
    if (snapshot.version > 0) {
      return snapshot.content;
    }
    return fallbackContent;
  }, [fallbackContent, snapshot.content, snapshot.version]);

  return (
    <section className="panel assistant-panel">
      <h3>Assistant Output (Stream)</h3>

      {history.length === 0 ? <div className="empty">还没有 AI 回复</div> : null}
      <div className="assistant-history">
        {history.map((item) => (
          <div key={item.id} className="assistant-line">
            {item.content}
          </div>
        ))}
      </div>

      <div className="assistant-live-wrap">
        <div className="assistant-live-label">Live</div>
        <pre className="assistant-live-text">{liveText}</pre>
      </div>
    </section>
  );
}

export default memo(AssistantStreamPane);
