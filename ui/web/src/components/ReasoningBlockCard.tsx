import { useEffect, useMemo, useState } from "react";
import type { ReasoningBlock } from "../types/chat";
import { RightArrowIcon } from "../pic/icon";

interface ReasoningBlockCardProps {
  block: ReasoningBlock;
}

export default function ReasoningBlockCard({ block }: ReasoningBlockCardProps) {
  const [collapsed, setCollapsed] = useState(block.collapsed);
  const [now, setNow] = useState(Date.now());

  // 思考中时需要实时更新计时
  useEffect(() => {
    if (block.closed) {
      return;
    }
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => clearInterval(timer);
  }, [block.closed]);

  const title = useMemo(() => {
    const startedAt = new Date(block.startedAt).getTime();
    const duration = formatDuration(startedAt, now);
    const prefix = block.closed ? "Thought" : "Thinking";
    return `${prefix} ${duration}`;
  }, [block.closed, block.startedAt, now]);

  return (
    <section className={`reasoning-block${collapsed ? "" : " expanded"}`}>
      <button
        type="button"
        className="reasoning-block-toggle"
        aria-expanded={!collapsed}
        onClick={() => setCollapsed((prev) => !prev)}
      >
        <span className="reasoning-block-title">
          {title}
          <RightArrowIcon
            className="reasoning-block-chevron"
            aria-hidden="true"
            width={14}
            height={14}
          />
        </span>
      </button>
      <div className={`reasoning-block-body${collapsed ? "" : " expanded"}`}>
        <div className="reasoning-block-content">{block.content}</div>
      </div>
    </section>
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
