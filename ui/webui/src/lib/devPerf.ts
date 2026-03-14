interface PerfSnapshot {
  deltaRecvCount: number;
  deltaCommitCount: number;
  commitMsTotal: number;
  assistantPaneRenders: number;
}

const ENABLED = resolveEnabled();
const bySession = new Map<string, PerfSnapshot>();
let timer: number | null = null;

export function recordDeltaRecv(sessionId: string): void {
  if (!ENABLED) {
    return;
  }
  ensureTimer();
  const next = getOrCreate(sessionId);
  next.deltaRecvCount += 1;
}

export function recordDeltaCommit(sessionId: string, durationMs: number): void {
  if (!ENABLED) {
    return;
  }
  ensureTimer();
  const next = getOrCreate(sessionId);
  next.deltaCommitCount += 1;
  next.commitMsTotal += Math.max(0, durationMs);
}

export function recordAssistantPaneRender(sessionId?: string): void {
  if (!ENABLED || !sessionId) {
    return;
  }
  ensureTimer();
  const next = getOrCreate(sessionId);
  next.assistantPaneRenders += 1;
}

function ensureTimer(): void {
  if (timer !== null || typeof window === "undefined") {
    return;
  }
  timer = window.setInterval(flush, 1000);
}

function flush(): void {
  for (const [sessionId, item] of bySession.entries()) {
    if (item.deltaRecvCount === 0 && item.deltaCommitCount === 0 && item.assistantPaneRenders === 0) {
      continue;
    }
    const avgCommit = item.deltaCommitCount > 0 ? item.commitMsTotal / item.deltaCommitCount : 0;
    console.debug("[webui_stream_perf]", {
      sessionId,
      delta_recv_count: item.deltaRecvCount,
      delta_commit_count: item.deltaCommitCount,
      commit_avg_ms: Number(avgCommit.toFixed(2)),
      assistant_pane_renders: item.assistantPaneRenders
    });
    bySession.set(sessionId, {
      deltaRecvCount: 0,
      deltaCommitCount: 0,
      commitMsTotal: 0,
      assistantPaneRenders: 0
    });
  }
}

function getOrCreate(sessionId: string): PerfSnapshot {
  const existing = bySession.get(sessionId);
  if (existing) {
    return existing;
  }
  const created: PerfSnapshot = {
    deltaRecvCount: 0,
    deltaCommitCount: 0,
    commitMsTotal: 0,
    assistantPaneRenders: 0
  };
  bySession.set(sessionId, created);
  return created;
}

function resolveEnabled(): boolean {
  const viteEnv =
    typeof import.meta !== "undefined" ? (import.meta as { env?: Record<string, unknown> }).env : undefined;
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEBUI_STREAM_PERF?: string | boolean;
          VITE_OPENJAX_WEBUI_STREAM_PERF?: string | boolean;
        })
      : {};
  const raw = String(
    viteEnv?.VITE_OPENJAX_WEBUI_STREAM_PERF ??
      globals.OPENJAX_WEBUI_STREAM_PERF ??
      globals.VITE_OPENJAX_WEBUI_STREAM_PERF ??
      "0"
  )
    .trim()
    .toLowerCase();
  return !(raw === "0" || raw === "false" || raw === "off" || raw === "disabled");
}
