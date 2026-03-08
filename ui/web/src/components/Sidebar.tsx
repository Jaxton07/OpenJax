import type { ChatSession } from "../types/chat";

interface SidebarProps {
  sessions: ChatSession[];
  activeSessionId: string | null;
  onNewChat: () => void;
  onSelectSession: (id: string) => void;
  onOpenSettings: () => void;
}

export default function Sidebar(props: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar-top">
        <button className="new-chat-btn" onClick={props.onNewChat}>
          + 新聊天
        </button>
      </div>
      <div className="history-list">
        {props.sessions.map((session) => (
          <button
            key={session.id}
            className={`history-item ${props.activeSessionId === session.id ? "active" : ""}`}
            onClick={() => props.onSelectSession(session.id)}
            title={session.title}
          >
            <span className="history-title">{session.title}</span>
            <span className="history-meta">{formatLocalTime(session.createdAt)}</span>
          </button>
        ))}
        {props.sessions.length === 0 ? (
          <div className="history-empty">暂无历史对话</div>
        ) : null}
      </div>
      <div className="sidebar-bottom">
        <button className="settings-btn" onClick={props.onOpenSettings}>
          设置
        </button>
      </div>
    </aside>
  );
}

function formatLocalTime(iso: string): string {
  const date = new Date(iso);
  return `${date.getMonth() + 1}-${date.getDate()} ${date
    .getHours()
    .toString()
    .padStart(2, "0")}:${date.getMinutes().toString().padStart(2, "0")}`;
}
