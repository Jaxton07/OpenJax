import { recordDeltaCommit } from "./devPerf";
import type { StreamStoreSnapshot } from "../types/chat";

interface StreamEntry {
  snapshot: StreamStoreSnapshot;
  pending: string;
  rafId: number | null;
  flushTimerId: number | null;
}

type Listener = () => void;

const EMPTY: StreamStoreSnapshot = {
  content: "",
  lastEventSeq: 0,
  isActive: false,
  version: 0
};
const MAX_CHARS = 10;
const MAX_WAIT_MS = 50;

const entries = new Map<string, StreamEntry>();
const listeners = new Map<string, Set<Listener>>();

export const streamStore = {
  start,
  append,
  complete,
  fail,
  clearSession,
  subscribe,
  getSnapshot,
  __resetForTests
};

function start(sessionId: string, turnId: string | undefined, seq: number, initialContent = ""): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const prev = entries.get(key);
  if (prev && seq <= prev.snapshot.lastEventSeq) {
    return;
  }
  const content = prev?.snapshot.content.length ? prev.snapshot.content : initialContent;
  entries.set(key, {
    snapshot: {
      turnId,
      content,
      lastEventSeq: Math.max(prev?.snapshot.lastEventSeq ?? 0, seq),
      isActive: true,
      version: (prev?.snapshot.version ?? 0) + 1
    },
    pending: "",
    rafId: prev?.rafId ?? null,
    flushTimerId: prev?.flushTimerId ?? null
  });
  notify(key);
}

function append(sessionId: string, turnId: string | undefined, delta: string, seq: number): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  if (!entries.has(key)) {
    start(sessionId, turnId, seq, "");
  }
  const entry = entries.get(key);
  if (!entry) {
    return;
  }
  if (seq <= entry.snapshot.lastEventSeq) {
    return;
  }
  entry.snapshot.lastEventSeq = seq;
  if (delta.length > 0) {
    entry.pending += delta;
  }
  scheduleCommit(key, sessionId, false);
}

function complete(sessionId: string, turnId: string | undefined, content: string | undefined, seq: number): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  if (!entries.has(key)) {
    start(sessionId, turnId, seq, String(content ?? ""));
  }
  const entry = entries.get(key);
  if (!entry) {
    return;
  }
  if (seq <= entry.snapshot.lastEventSeq) {
    return;
  }

  cancelCommit(entry);
  cancelFlushTimer(entry);
  const merged = entry.pending.length > 0 ? `${entry.snapshot.content}${entry.pending}` : entry.snapshot.content;
  const resolved = content && content.length > 0 ? content : merged;

  entry.pending = "";
  entry.snapshot = {
    ...entry.snapshot,
    content: resolved,
    lastEventSeq: seq,
    isActive: false,
    version: entry.snapshot.version + 1
  };
  notify(key);
}

function fail(sessionId: string, turnId: string | undefined, seq: number): void {
  if (!turnId) {
    return;
  }
  const key = composeKey(sessionId, turnId);
  const entry = entries.get(key);
  if (!entry) {
    return;
  }
  cancelCommit(entry);
  cancelFlushTimer(entry);
  entry.pending = "";
  entry.snapshot = {
    ...entry.snapshot,
    lastEventSeq: Math.max(entry.snapshot.lastEventSeq, seq),
    isActive: false,
    version: entry.snapshot.version + 1
  };
  notify(key);
}

function clearSession(sessionId: string): void {
  const prefix = `${sessionId}::`;
  for (const key of [...entries.keys()]) {
    if (!key.startsWith(prefix)) {
      continue;
    }
    const entry = entries.get(key);
    if (entry) {
      cancelCommit(entry);
      cancelFlushTimer(entry);
    }
    entries.delete(key);
    notify(key);
  }
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
    const current = listeners.get(key);
    if (!current) {
      return;
    }
    current.delete(listener);
    if (current.size === 0) {
      listeners.delete(key);
    }
  };
}

function getSnapshot(sessionId: string | undefined, turnId: string | undefined): StreamStoreSnapshot {
  if (!sessionId || !turnId) {
    return EMPTY;
  }
  return entries.get(composeKey(sessionId, turnId))?.snapshot ?? EMPTY;
}

function scheduleCommit(key: string, sessionId: string, force: boolean): void {
  const entry = entries.get(key);
  if (!entry || entry.rafId !== null) {
    if (!entry) {
      return;
    }
    if (!force && entry.flushTimerId === null) {
      entry.flushTimerId = setTimeout(() => {
        const current = entries.get(key);
        if (!current) {
          return;
        }
        current.flushTimerId = null;
        scheduleCommit(key, sessionId, true);
      }, MAX_WAIT_MS) as unknown as number;
    }
    return;
  }
  entry.rafId = resolveRaf()(() => {
    const current = entries.get(key);
    if (!current) {
      return;
    }
    current.rafId = null;
    if (!current.pending.length) {
      return;
    }

    const picked = pickNextChunk(current.pending, force);
    if (!picked.chunk.length) {
      if (!force && current.flushTimerId === null) {
        current.flushTimerId = setTimeout(() => {
          const latest = entries.get(key);
          if (!latest) {
            return;
          }
          latest.flushTimerId = null;
          scheduleCommit(key, sessionId, true);
        }, MAX_WAIT_MS) as unknown as number;
      }
      return;
    }

    const started = nowMs();
    current.snapshot = {
      ...current.snapshot,
      content: `${current.snapshot.content}${picked.chunk}`,
      version: current.snapshot.version + 1
    };
    current.pending = picked.rest;
    const ended = nowMs();
    recordDeltaCommit(sessionId, ended - started);
    notify(key);

    if (!current.pending.length) {
      cancelFlushTimer(current);
      return;
    }
    scheduleCommit(key, sessionId, false);
  });
}

function cancelCommit(entry: StreamEntry): void {
  if (entry.rafId === null) {
    return;
  }
  resolveCancelRaf()(entry.rafId);
  entry.rafId = null;
}

function cancelFlushTimer(entry: StreamEntry): void {
  if (entry.flushTimerId === null) {
    return;
  }
  clearTimeout(entry.flushTimerId);
  entry.flushTimerId = null;
}

function notify(key: string): void {
  const group = listeners.get(key);
  if (!group) {
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

function pickNextChunk(
  pending: string,
  force: boolean
): { chunk: string; rest: string } {
  const chars = Array.from(pending);
  if (chars.length === 0) {
    return { chunk: "", rest: "" };
  }

  const scanLimit = Math.min(chars.length, MAX_CHARS);
  for (let i = 0; i < scanLimit; i += 1) {
    // Keep explicit line breaks as a natural chunk boundary.
    if (chars[i] !== "\n") {
      continue;
    }
    const take = i + 1;
    return {
      chunk: chars.slice(0, take).join(""),
      rest: chars.slice(take).join("")
    };
  }
  const take = Math.min(chars.length, MAX_CHARS);
  return {
    chunk: chars.slice(0, take).join(""),
    rest: chars.slice(take).join("")
  };
}

function __resetForTests(): void {
  for (const entry of entries.values()) {
    cancelCommit(entry);
    cancelFlushTimer(entry);
  }
  entries.clear();
  listeners.clear();
}
