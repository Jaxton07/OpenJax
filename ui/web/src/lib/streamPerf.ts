export interface StreamPerfSnapshot {
  deltaRecvCount: number;
  deltaCommitCount: number;
  commitMsTotal: number;
  messageListRenderCount: number;
}

const STREAM_PERF_ENABLED = resolveStreamPerfEnabled();

const perfBySession = new Map<string, StreamPerfSnapshot>();
let flushTimer: number | null = null;

export function recordDeltaReceived(sessionId: string): void {
  if (!STREAM_PERF_ENABLED) {
    return;
  }
  ensureFlushTimer();
  const next = getOrCreate(sessionId);
  next.deltaRecvCount += 1;
}

export function recordDeltaCommitted(sessionId: string, durationMs: number): void {
  if (!STREAM_PERF_ENABLED) {
    return;
  }
  ensureFlushTimer();
  const next = getOrCreate(sessionId);
  next.deltaCommitCount += 1;
  next.commitMsTotal += Math.max(0, durationMs);
}

export function recordMessageListRender(sessionId: string | undefined): void {
  if (!STREAM_PERF_ENABLED || !sessionId) {
    return;
  }
  ensureFlushTimer();
  const next = getOrCreate(sessionId);
  next.messageListRenderCount += 1;
}

function ensureFlushTimer(): void {
  if (flushTimer !== null || typeof window === "undefined") {
    return;
  }
  flushTimer = window.setInterval(flushPerfMetrics, 1000);
}

function flushPerfMetrics(): void {
  for (const [sessionId, snapshot] of perfBySession.entries()) {
    if (
      snapshot.deltaRecvCount === 0 &&
      snapshot.deltaCommitCount === 0 &&
      snapshot.messageListRenderCount === 0
    ) {
      continue;
    }
    const avgCommitMs =
      snapshot.deltaCommitCount > 0 ? snapshot.commitMsTotal / snapshot.deltaCommitCount : 0;
    console.debug("[stream_perf]", {
      sessionId,
      delta_recv_count: snapshot.deltaRecvCount,
      delta_commit_count: snapshot.deltaCommitCount,
      commit_avg_ms: Number(avgCommitMs.toFixed(2)),
      commit_per_sec: snapshot.deltaCommitCount,
      message_list_renders_per_sec: snapshot.messageListRenderCount
    });
    perfBySession.set(sessionId, {
      deltaRecvCount: 0,
      deltaCommitCount: 0,
      commitMsTotal: 0,
      messageListRenderCount: 0
    });
  }
}

function getOrCreate(sessionId: string): StreamPerfSnapshot {
  const existing = perfBySession.get(sessionId);
  if (existing) {
    return existing;
  }
  const created: StreamPerfSnapshot = {
    deltaRecvCount: 0,
    deltaCommitCount: 0,
    commitMsTotal: 0,
    messageListRenderCount: 0
  };
  perfBySession.set(sessionId, created);
  return created;
}

function resolveStreamPerfEnabled(): boolean {
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_STREAM_PERF?: string | boolean;
          VITE_OPENJAX_WEB_STREAM_PERF?: string | boolean;
        })
      : {};
  const raw = String(
    globals.OPENJAX_WEB_STREAM_PERF ??
      globals.VITE_OPENJAX_WEB_STREAM_PERF ??
      "0"
  )
    .trim()
    .toLowerCase();
  return !(raw === "0" || raw === "off" || raw === "false" || raw === "disabled");
}
