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

function parseContextUsage(event: StreamEvent) {
  const rawInputTokens = Number(event.payload.input_tokens ?? event.payload.inputTokens ?? 0);
  const rawContextWindowSize = Number(
    event.payload.context_window_size ?? event.payload.contextWindowSize ?? 0
  );
  const fallbackRatio =
    rawContextWindowSize > 0 ? rawInputTokens / rawContextWindowSize : Number(event.payload.ratio ?? 0);
  const ratio = Number.isFinite(fallbackRatio) ? Math.max(0, Math.min(1, fallbackRatio)) : 0;

  return {
    ratio,
    inputTokens: Number.isFinite(rawInputTokens) ? rawInputTokens : 0,
    contextWindowSize: Number.isFinite(rawContextWindowSize) ? rawContextWindowSize : 0,
    updatedAt: String(event.payload.updated_at ?? event.payload.updatedAt ?? event.timestamp)
  };
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
  if (event.type === "user_message") {
    const content = String(event.payload.content ?? "");
    const existingIdx = next.messages.findIndex(
      (message) =>
        message.kind === "text" &&
        message.role === "user" &&
        message.content === content &&
        message.turnId === turnId
    );
    if (existingIdx >= 0) {
      next.messages[existingIdx] = {
        ...next.messages[existingIdx],
        timestamp: event.timestamp,
        startEventSeq: next.messages[existingIdx].startEventSeq ?? event.event_seq,
        lastEventSeq: Math.max(next.messages[existingIdx].lastEventSeq ?? event.event_seq, event.event_seq),
        turnId
      };
    } else {
      next.messages.push({
        id: crypto.randomUUID(),
        kind: "text",
        role: "user",
        content,
        timestamp: event.timestamp,
        startEventSeq: event.event_seq,
        lastEventSeq: event.event_seq,
        turnId
      });
    }
  }

  if (turnId && shouldCloseReasoningOnEvent(event)) {
    closeOpenReasoningBlock(next.messages, turnId, event.event_seq, event.timestamp);
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
    // Legacy compat only: assistant_message seeds draft text but does not finalize the turn.
    finalizeAssistantMessageFallback(next.messages, turnId, String(event.payload.content ?? ""), event.timestamp, event);
  }

  if (event.type === "response_completed" && turnId) {
    sealAssistantMessage(next.messages, turnId, String(event.payload.content ?? ""), event.timestamp, event);
  }

  if (event.type === "turn_interrupted" && turnId) {
    for (let i = 0; i < next.messages.length; i += 1) {
      const message = next.messages[i];
      if (message.role !== "assistant") {
        continue;
      }
      if (turnId && message.turnId !== turnId) {
        continue;
      }
      next.messages[i] = {
        ...message,
        isDraft: false,
        interrupted: true,
        lastEventSeq: Math.max(message.lastEventSeq ?? event.event_seq, event.event_seq)
      };
    }
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

  if (event.type === "context_usage_updated") {
    next.contextUsage = parseContextUsage(event);
  }

  if (event.type === "turn_completed" || event.type === "response_completed") {
    next.turnPhase = "completed";
    markTurnDraftFinal(next.messages, turnId);
  }
  if (event.type === "turn_interrupted") {
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
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      turnId,
      timestamp: event.timestamp
    });
  }

  return next;
}
