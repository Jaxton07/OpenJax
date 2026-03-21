import type { ContextUsageState } from "../../types/chat";

interface ContextUsageRingProps {
  contextUsage?: ContextUsageState | null;
}

function clampRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) {
    return 0;
  }
  return Math.max(0, Math.min(1, ratio));
}

function formatCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "0";
  }
  return Math.max(0, Math.round(value)).toLocaleString("en-US");
}

function formatPercent(ratio: number): string {
  return `${(clampRatio(ratio) * 100).toFixed(1)}%`;
}

export default function ContextUsageRing({ contextUsage }: ContextUsageRingProps) {
  const hasUsage =
    !!contextUsage &&
    Number.isFinite(contextUsage.inputTokens) &&
    Number.isFinite(contextUsage.contextWindowSize) &&
    contextUsage.contextWindowSize > 0;
  const ratio = clampRatio(contextUsage?.ratio ?? 0);
  const percentLabel = formatPercent(ratio);
  const tokenLabel = hasUsage
    ? `${formatCount(contextUsage.inputTokens)} / ${formatCount(contextUsage.contextWindowSize)} tokens`
    : "暂无上下文数据";
  const title = hasUsage ? `上下文使用 ${percentLabel} · ${tokenLabel}` : "上下文用量暂无数据";
  const strokeRatio = hasUsage ? ratio : 0;

  return (
    <div
      className={`context-usage-ring${hasUsage ? "" : " is-empty"}`}
      aria-label={title}
      title={title}
      role="img"
    >
      <svg viewBox="0 0 36 36" aria-hidden="true" focusable="false">
        <circle className="context-usage-ring-track" cx="18" cy="18" r="14" pathLength="100" />
        <circle
          className="context-usage-ring-progress"
          cx="18"
          cy="18"
          r="14"
          pathLength="100"
          transform="rotate(-90 18 18)"
          strokeDasharray={`${strokeRatio * 100} ${100 - strokeRatio * 100}`}
        />
      </svg>
    </div>
  );
}
