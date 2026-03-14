import { useCallback, useSyncExternalStore } from "react";
import { streamStore } from "../lib/streamStore";
import type { StreamStoreSnapshot } from "../types/chat";

const EMPTY: StreamStoreSnapshot = {
  content: "",
  lastEventSeq: 0,
  isActive: false,
  version: 0
};

export function useStreamSnapshot(sessionId: string | undefined, turnId: string | undefined): StreamStoreSnapshot {
  const subscribe = useCallback((listener: () => void) => streamStore.subscribe(sessionId, turnId, listener), [sessionId, turnId]);
  const getSnapshot = useCallback(() => streamStore.getSnapshot(sessionId, turnId), [sessionId, turnId]);
  const getServerSnapshot = useCallback(() => EMPTY, []);
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
