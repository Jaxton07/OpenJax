import { useState } from "react";
import { SettingsIcon, TrashIcon } from "../pic/icon";
import type { ChatSession } from "../types/chat";

interface SidebarProps {
  sessions: ChatSession[];
  activeSessionId: string | null;
  collapsed: boolean;
  onSelectSession: (id: string) => void;
  onDeleteSession: (id: string) => Promise<void> | void;
  onOpenSettings: () => void;
  onManageSessions: () => void;
  onLogout: () => void;
}

export default function Sidebar(props: SidebarProps) {
  const [deletingSessionId, setDeletingSessionId] = useState<string | null>(null);

  const handleDeleteSession = async (sessionId: string) => {
    if (deletingSessionId === sessionId) {
      return;
    }
    setDeletingSessionId(sessionId);
    try {
      await props.onDeleteSession(sessionId);
    } finally {
      setDeletingSessionId((current) => (current === sessionId ? null : current));
    }
  };

  return (
    <aside className={`sidebar ${props.collapsed ? "collapsed" : ""}`}>
      <div className="history-list">
        {props.sessions.map((session) => (
          <div
            key={session.id}
            className={`history-item ${props.activeSessionId === session.id ? "active" : ""}`}
          >
            <button
              className="history-main"
              onClick={() => props.onSelectSession(session.id)}
              title={session.title}
            >
              <span className="history-title">{session.title}</span>
              <span className="history-meta">{formatLocalTime(session.createdAt)}</span>
            </button>
            <button
              className="history-delete-btn"
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                void handleDeleteSession(session.id);
              }}
              disabled={deletingSessionId === session.id}
              title="删除会话"
              aria-label="删除会话"
            >
              <TrashIcon aria-hidden="true" />
            </button>
          </div>
        ))}
        {props.sessions.length === 0 ? <div className="history-empty">暂无历史对话</div> : null}
      </div>
      <div className="sidebar-bottom">
        <button className="settings-btn" onClick={props.onOpenSettings}>
          <span className="btn-icon" aria-hidden="true">
            <SettingsIcon />
          </span>
          <span>设置</span>
        </button>
        <button className="settings-btn" onClick={props.onManageSessions}>
          <span>会话管理</span>
        </button>
        <button className="logout-btn" onClick={props.onLogout}>
          <span>退出登录</span>
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
