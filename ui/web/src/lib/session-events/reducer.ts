import type { ChatSession } from "../../types/chat";
import type { StreamEvent } from "../../types/gateway";
import {
  appendReasoningDelta,
  closeOpenReasoningBlock,
  finalizeAssistantMessageFallback,
  isCanonicalDeltaEvent,
  markTurnDraftFinal,
  mergeAssistantDraft,
  sealAssistantMessage,
  shouldCloseReasoningOnEvent
} from "./assistant";
import { looksLikeSequenceReset } from "./sequence";
import {
  batchStatusFromPayload,
  isToolStepEvent,
  toolCallIdFromPayload,
  upsertToolBatchSummary,
  upsertToolStepMessage
} from "./tools";

export function applySessionEvent(session: ChatSession, event: StreamEvent): ChatSession {
  return applySessionEvents(session, [event]);
}

export function applySessionEvents(session: ChatSession, events: StreamEvent[]): ChatSession {
  let current = session;
  for (const event of events) {
    current = applySingleSessionEvent(current, event);
  }
  return current;
}

function applySingleSessionEvent(session: ChatSession, event: StreamEvent): ChatSession {
  if (event.event_seq <= session.lastEventSeq && !looksLikeSequenceReset(session, event)) {
    return session;
  }

  const next: ChatSession = {
    ...session,
    lastEventSeq: event.event_seq,
    pendingApprovals: [...session.pendingApprovals],
    messages: [...session.messages]
  };

  const turnId = event.turn_id;
  if (turnId && shouldCloseReasoningOnEvent(event)) {
    closeOpenReasoningBlock(next.messages, turnId);
  }
  if (event.type === "turn_started" || event.type === "response_started" || event.type === "response_resumed") {
    next.turnPhase = "streaming";
  }

  if (event.type === "reasoning_delta" && turnId) {
    appendReasoningDelta(
      next.messages,
      turnId,
      String(event.payload.content_delta ?? ""),
      event.timestamp,
      event.event_seq
    );
  }

  if (event.type === "response_text_delta" && turnId) {
    mergeAssistantDraft(
      next.messages,
      turnId,
      String(event.payload.content_delta ?? ""),
      event.timestamp,
      isCanonicalDeltaEvent(event),
      event
    );
  }

  if (event.type === "assistant_message" && turnId) {
    finalizeAssistantMessageFallback(next.messages, turnId, String(event.payload.content ?? ""), event.timestamp, event);
  }

  if (event.type === "response_completed" && turnId) {
    sealAssistantMessage(next.messages, turnId, String(event.payload.content ?? ""), event.timestamp, event);
  }

  if (event.type === "tool_calls_proposed") {
    upsertToolBatchSummary(next.messages, event, "running");
  }

  if (event.type === "tool_batch_completed") {
    upsertToolBatchSummary(next.messages, event, batchStatusFromPayload(event.payload));
  }

  if (isToolStepEvent(event)) {
    upsertToolStepMessage(next.messages, event);
  }

  if (event.type === "approval_requested") {
    next.pendingApprovals.push({
      approvalId: String(event.payload.approval_id ?? ""),
      toolCallId: toolCallIdFromPayload(event),
      turnId,
      target: String(event.payload.target ?? ""),
      reason: String(event.payload.reason ?? ""),
      toolName: String(event.payload.tool_name ?? "")
    });
  }

  if (event.type === "approval_resolved") {
    const approvalId = String(event.payload.approval_id ?? "");
    next.pendingApprovals = next.pendingApprovals.filter((item) => item.approvalId !== approvalId);
  }

  if (event.type === "turn_completed" || event.type === "response_completed") {
    next.turnPhase = "completed";
    markTurnDraftFinal(next.messages, turnId);
  }

  if (event.type === "session_shutdown") {
    next.connection = "closed";
    if (next.turnPhase === "streaming") {
      next.turnPhase = "completed";
    }
    markTurnDraftFinal(next.messages, turnId);
  }

  if (event.type === "error" || event.type === "response_error") {
    next.turnPhase = "failed";
    next.messages.push({
      id: crypto.randomUUID(),
      kind: "text",
      role: "error",
      content: String(event.payload.message ?? "turn failed"),
      turnId,
      timestamp: event.timestamp
    });
  }

  return next;
}
