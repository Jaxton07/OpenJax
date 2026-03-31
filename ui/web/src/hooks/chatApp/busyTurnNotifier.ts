export const BUSY_TURN_BLOCKED_MESSAGE = "Please wait for the current response to finish.";
export const BUSY_TURN_TOAST_DEDUP_MS = 1500;

export function createBusyTurnNotifier(
  emit: (message: string) => void,
  dedupMs = BUSY_TURN_TOAST_DEDUP_MS
): () => boolean {
  let lastEmittedAt = 0;
  return () => {
    const now = Date.now();
    if (now - lastEmittedAt < dedupMs) {
      return false;
    }
    lastEmittedAt = now;
    emit(BUSY_TURN_BLOCKED_MESSAGE);
    return true;
  };
}

