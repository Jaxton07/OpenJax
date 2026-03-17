import { useEffect, useMemo, useState } from "react";
import type { ReasoningBlock } from "../types/chat";

interface ReasoningBlockCardProps {
  block: ReasoningBlock;
}

export default function ReasoningBlockCard({ block }: ReasoningBlockCardProps) {
  const [collapsed, setCollapsed] = useState(block.collapsed);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (block.closed) {
      return;
    }
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => clearInterval(timer);
  }, [block.closed]);

  const duration = useMemo(() => {
    const startedAt = new Date(block.startedAt).getTime();
    const endTime =
      block.closed && block.endedAt ? new Date(block.endedAt).getTime() : now;
    return formatDuration(startedAt, endTime);
  }, [block, now]);

  const isActive = !block.closed;

  const blockClass = [
    "reasoning-block",
    !collapsed ? "open" : "",
    isActive ? "active" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={blockClass}
      data-testid="reasoning-block"
      onClick={() => !isActive && setCollapsed((prev) => !prev)}
      role={isActive ? undefined : "button"}
      aria-expanded={isActive ? undefined : !collapsed}
      tabIndex={isActive ? undefined : 0}
      onKeyDown={
        isActive
          ? undefined
          : (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setCollapsed((prev) => !prev);
              }
            }
      }
    >
      <div className="reasoning-block-header">
        <span className="reasoning-block-label">思考过程</span>
        <span className="reasoning-block-dur">{duration}</span>
        {!isActive && (
          <svg
            className="reasoning-block-chevron"
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <polyline points="18 15 12 9 6 15" />
          </svg>
        )}
      </div>
      <div className="reasoning-block-body">
        <div className="reasoning-block-content">{block.content}</div>
      </div>
    </div>
  );
}

function formatDuration(startMs: number, endMs: number): string {
  const diffSec = Math.max(0, Math.floor((endMs - startMs) / 1000));
  const minutes = Math.floor(diffSec / 60);
  const seconds = diffSec % 60;
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}
