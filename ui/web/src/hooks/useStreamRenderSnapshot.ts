import { useCallback, useSyncExternalStore } from "react";
import { streamRenderStore } from "../lib/streamRenderStore";
import type { StreamRenderSnapshot } from "../lib/streamRenderStore";

const EMPTY_SNAPSHOT: StreamRenderSnapshot = {
  content: "",
  lastEventSeq: 0,
  version: 0,
  isActive: false
};

export function useStreamRenderSnapshot(
  sessionId: string | undefined,
  turnId: string | undefined
): StreamRenderSnapshot {
  const subscribe = useCallback(
    (listener: () => void) => streamRenderStore.subscribe(sessionId, turnId, listener),
    [sessionId, turnId]
  );
  const getSnapshot = useCallback(() => streamRenderStore.getSnapshot(sessionId, turnId), [sessionId, turnId]);
  const getServerSnapshot = useCallback(() => EMPTY_SNAPSHOT, []);
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
