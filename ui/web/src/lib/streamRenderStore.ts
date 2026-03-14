import { recordDeltaCommitted } from "./streamPerf";

export interface StreamRenderSnapshot {
  turnId?: string;
  messageId?: string;
  content: string;
  lastEventSeq: number;
  version: number;
  isActive: boolean;
}

type Listener = () => void;

interface StreamRenderEntry {
  snapshot: StreamRenderSnapshot;
  pendingBuffer: string;
  rafId: number | null;
}

const EMPTY_SNAPSHOT: StreamRenderSnapshot = {
  content: "",
  lastEventSeq: 0,
  version: 0,
  isActive: false
};

const store = new Map<string, StreamRenderEntry>();
const listeners = new Map<string, Set<Listener>>();

export const streamRenderStore = {
  start,
  append,
  complete,
  fail,
  clear,
  subscribe,
  getSnapshot,
  hasActiveTurn,
  __dangerousResetForTests
};

function start(
  sessionId: string,
  turnId: string | undefined,
  messageId: string | undefined,
  seq: number,
  initialContent = ""
): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const existing = store.get(key);
  if (existing && seq <= existing.snapshot.lastEventSeq) {
    return;
  }
  const nextContent = existing?.snapshot.content || initialContent;
  const nextVersion = (existing?.snapshot.version ?? 0) + 1;
  store.set(key, {
    snapshot: {
      turnId,
      messageId: messageId ?? existing?.snapshot.messageId,
      content: nextContent,
      lastEventSeq: Math.max(existing?.snapshot.lastEventSeq ?? 0, seq),
      version: nextVersion,
      isActive: true
    },
    pendingBuffer: "",
    rafId: existing?.rafId ?? null
  });
  notify(key);
}

function append(sessionId: string, turnId: string | undefined, delta: string, seq: number): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const existing = store.get(key);
  if (!existing) {
    start(sessionId, turnId, undefined, seq, "");
  }
  const current = store.get(key);
  if (!current) {
    return;
  }
  if (seq <= current.snapshot.lastEventSeq) {
    return;
  }

  current.snapshot.lastEventSeq = seq;
  if (delta.length > 0) {
    current.pendingBuffer += delta;
  }
  scheduleCommit(key, sessionId);
}

function complete(
  sessionId: string,
  turnId: string | undefined,
  finalContent: string | undefined,
  seq: number
): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const current = store.get(key);
  if (!current) {
    start(sessionId, turnId, undefined, seq, String(finalContent ?? ""));
  }
  const next = store.get(key);
  if (!next) {
    return;
  }
  if (seq <= next.snapshot.lastEventSeq) {
    return;
  }
  cancelCommit(next);
  const buffered = next.pendingBuffer.length > 0 ? `${next.snapshot.content}${next.pendingBuffer}` : next.snapshot.content;
  const resolved = typeof finalContent === "string" && finalContent.length > 0 ? finalContent : buffered;
  next.pendingBuffer = "";
  next.snapshot = {
    ...next.snapshot,
    content: resolved,
    lastEventSeq: seq,
    version: next.snapshot.version + 1,
    isActive: false
  };
  notify(key);
}

function fail(sessionId: string, turnId: string | undefined, seq: number): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const current = store.get(key);
  if (!current) {
    return;
  }
  cancelCommit(current);
  current.pendingBuffer = "";
  current.snapshot = {
    ...current.snapshot,
    lastEventSeq: Math.max(current.snapshot.lastEventSeq, seq),
    version: current.snapshot.version + 1,
    isActive: false
  };
  notify(key);
}

function clear(sessionId: string, turnId?: string): void {
  if (turnId) {
    clearOne(composeKey(sessionId, turnId));
    return;
  }
  const prefix = `${sessionId}::`;
  for (const key of [...store.keys()]) {
    if (key.startsWith(prefix)) {
      clearOne(key);
    }
  }
}

function clearOne(key: string): void {
  const current = store.get(key);
  if (!current) {
    return;
  }
  cancelCommit(current);
  store.delete(key);
  notify(key);
}

function subscribe(sessionId: string | undefined, turnId: string | undefined, listener: Listener): () => void {
  if (!sessionId || !turnId) {
    return () => {};
  }
  const key = composeKey(sessionId, turnId);
  const group = listeners.get(key) ?? new Set<Listener>();
  group.add(listener);
  listeners.set(key, group);
  return () => {
    const next = listeners.get(key);
    if (!next) {
      return;
    }
    next.delete(listener);
    if (next.size === 0) {
      listeners.delete(key);
    }
  };
}

function getSnapshot(sessionId: string | undefined, turnId: string | undefined): StreamRenderSnapshot {
  if (!sessionId || !turnId) {
    return EMPTY_SNAPSHOT;
  }
  return store.get(composeKey(sessionId, turnId))?.snapshot ?? EMPTY_SNAPSHOT;
}

function hasActiveTurn(sessionId: string | undefined, turnId: string | undefined): boolean {
  if (!sessionId || !turnId) {
    return false;
  }
  return store.get(composeKey(sessionId, turnId))?.snapshot.isActive ?? false;
}

function scheduleCommit(key: string, sessionId: string): void {
  const entry = store.get(key);
  if (!entry || entry.rafId !== null) {
    return;
  }
  const raf = resolveRaf();
  entry.rafId = raf(() => {
    const current = store.get(key);
    if (!current) {
      return;
    }
    current.rafId = null;
    if (!current.pendingBuffer) {
      return;
    }
    const startedAt = nowMs();
    current.snapshot = {
      ...current.snapshot,
      content: `${current.snapshot.content}${current.pendingBuffer}`,
      version: current.snapshot.version + 1
    };
    current.pendingBuffer = "";
    const endedAt = nowMs();
    recordDeltaCommitted(sessionId, endedAt - startedAt);
    notify(key);
  });
}

function cancelCommit(entry: StreamRenderEntry): void {
  if (entry.rafId === null) {
    return;
  }
  const cancelRaf = resolveCancelRaf();
  cancelRaf(entry.rafId);
  entry.rafId = null;
}

function notify(key: string): void {
  const group = listeners.get(key);
  if (!group || group.size === 0) {
    return;
  }
  for (const listener of [...group]) {
    listener();
  }
}

function composeKey(sessionId: string, turnId: string): string {
  return `${sessionId}::${turnId}`;
}

function resolveRaf(): (cb: FrameRequestCallback) => number {
  if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
    return window.requestAnimationFrame.bind(window);
  }
  return (cb: FrameRequestCallback) => setTimeout(() => cb(nowMs()), 16) as unknown as number;
}

function resolveCancelRaf(): (id: number) => void {
  if (typeof window !== "undefined" && typeof window.cancelAnimationFrame === "function") {
    return window.cancelAnimationFrame.bind(window);
  }
  return (id: number) => clearTimeout(id);
}

function nowMs(): number {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}

function __dangerousResetForTests(): void {
  for (const entry of store.values()) {
    cancelCommit(entry);
  }
  store.clear();
  listeners.clear();
}
