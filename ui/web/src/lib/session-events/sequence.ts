import type { ChatSession } from "../../types/chat";
import type { StreamEvent } from "../../types/gateway";

export function isSequenceResetEvent(event: StreamEvent): boolean {
  if (event.event_seq === 1) {
    return true;
  }
  return event.turn_seq === 1 && event.type === "response_started";
}

export function looksLikeSequenceReset(session: ChatSession, event: StreamEvent): boolean {
  if (session.lastEventSeq <= 0) {
    return false;
  }
  if (event.event_seq === 1) {
    return true;
  }
  if (event.turn_seq === 1 && (event.type === "turn_started" || event.type === "response_started")) {
    return true;
  }
  return false;
}
