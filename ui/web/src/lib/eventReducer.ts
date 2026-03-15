import type { ChatMessage, ChatSession, ReasoningBlock, ToolStep } from "../types/chat";
import type { StreamEvent } from "../types/gateway";

export function applyStreamEvent(session: ChatSession, event: StreamEvent): ChatSession {
  return applyStreamEvents(session, [event]);
}

export function applyStreamEvents(session: ChatSession, events: StreamEvent[]): ChatSession {
  let current = session;
  for (const event of events) {
    current = applySingleStreamEvent(current, event);
  }
  return current;
}

function applySingleStreamEvent(session: ChatSession, event: StreamEvent): ChatSession {
  if (event.event_seq <= session.lastEventSeq) {
    if (!looksLikeSequenceReset(session, event)) {
      return session;
    }
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
    const contentDelta = String(event.payload.content_delta ?? "");
    appendReasoningDelta(next.messages, turnId, contentDelta, event.timestamp, event.event_seq);
  }

  if (event.type === "response_text_delta" && turnId) {
    const contentDelta = String(event.payload.content_delta ?? "");
    mergeAssistantDraft(next.messages, turnId, contentDelta, event.timestamp, isCanonicalDeltaEvent(event), event);
  }

  if (event.type === "assistant_message" && turnId) {
    const content = String(event.payload.content ?? "");
    finalizeAssistantMessageFallback(next.messages, turnId, content, event.timestamp, event);
  }

  if (event.type === "response_completed" && turnId) {
    const content = String(event.payload.content ?? "");
    sealAssistantMessage(next.messages, turnId, content, event.timestamp, event);
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
    const message = String(event.payload.message ?? "turn failed");
    next.messages.push({
      id: crypto.randomUUID(),
      kind: "text",
      role: "error",
      content: message,
      turnId,
      timestamp: event.timestamp
    });
  }

  return next;
}

function looksLikeSequenceReset(session: ChatSession, event: StreamEvent): boolean {
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

function isToolStepEvent(event: StreamEvent): boolean {
  return (
    event.type === "tool_call_started" ||
    event.type === "tool_call_completed" ||
    event.type === "approval_requested" ||
    event.type === "approval_resolved" ||
    event.type === "error"
  );
}

function shouldCloseReasoningOnEvent(event: StreamEvent): boolean {
  return (
    event.type === "response_text_delta" ||
    event.type === "tool_call_started" ||
    event.type === "tool_calls_proposed" ||
    event.type === "tool_batch_completed" ||
    event.type === "response_completed" ||
    event.type === "response_error" ||
    event.type === "turn_completed"
  );
}

function upsertToolStepMessage(messages: ChatMessage[], event: StreamEvent): void {
  maybeWarnMissingToolCallId(event);
  const nextStep = createStepFromEvent(event);
  const messageIdx = findToolStepMessageIndex(messages, event);
  if (messageIdx < 0) {
    messages.push({
      id: crypto.randomUUID(),
      kind: "tool_steps",
      role: "assistant",
      content: "",
      timestamp: event.timestamp,
      turnId: event.turn_id,
      isDraft: false,
      toolSteps: [nextStep]
    });
    return;
  }

  const prevMessage = messages[messageIdx];
  const prevStep = prevMessage.toolSteps?.[0];
  messages[messageIdx] = {
    ...prevMessage,
    timestamp: event.timestamp,
    toolSteps: [mergeToolStep(prevStep, nextStep)]
  };
}

function findToolStepMessageIndex(messages: ChatMessage[], event: StreamEvent): number {
  const key = resolveAggregationKey(event);
  // Rules:
  // 1) same tool_call_id => same card
  // 2) do not merge across turns
  // 3) approval events fallback to approval_id when tool_call_id is missing
  // 4) no key means no cross-event merge (safe fallback)
  if (!key) {
    return -1;
  }

  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    const step = message.toolSteps?.[0];
    if (message.kind !== "tool_steps" || message.turnId !== event.turn_id || !step) {
      continue;
    }
    if (key.type === "tool_call_id" && step.toolCallId === key.value) {
      return i;
    }
    if (key.type === "approval_id" && step.approvalId === key.value) {
      return i;
    }
  }
  return -1;
}

function mergeToolStep(previous: ToolStep | undefined, next: ToolStep): ToolStep {
  if (!previous) {
    return next;
  }
  // Keep existing tool identity when approval events update the same tool call card.
  if (previous.type === "tool" && next.type === "approval") {
    return withDuration({
      ...previous,
      status: next.status,
      time: next.time,
      description: next.description ?? previous.description,
      approvalId: next.approvalId ?? previous.approvalId,
      toolCallId: next.toolCallId ?? previous.toolCallId,
      endedAt: next.endedAt ?? previous.endedAt,
      meta: next.meta
    });
  }
  return withDuration({ ...previous, ...next });
}

type AggregateKey = { type: "tool_call_id" | "approval_id"; value: string } | null;

function resolveAggregationKey(event: StreamEvent): AggregateKey {
  const toolCallId = toolCallIdFromPayload(event);
  if (toolCallId.length > 0) {
    return { type: "tool_call_id", value: toolCallId };
  }
  if (event.type === "approval_requested" || event.type === "approval_resolved") {
    const approvalId = String(event.payload.approval_id ?? "");
    if (approvalId.length > 0) {
      return { type: "approval_id", value: approvalId };
    }
  }
  return null;
}

function maybeWarnMissingToolCallId(event: StreamEvent): void {
  if (event.type !== "tool_call_started" && event.type !== "tool_call_completed") {
    return;
  }
  if (toolCallIdFromPayload(event).length > 0) {
    return;
  }
  console.debug("[tool_steps] missing tool_call_id; event will not be merged", {
    type: event.type,
    turn_id: event.turn_id,
    event_seq: event.event_seq,
    tool_name: event.payload.tool_name
  });
}

function createStepFromEvent(event: StreamEvent): ToolStep {
  if (event.type === "tool_call_started") {
    return {
      id: resolveToolStepId(event, "tool_call_started"),
      type: "tool",
      title: String(event.payload.tool_name ?? "tool"),
      subtitle: String(event.payload.target ?? ""),
      status: "running",
      time: event.timestamp,
      startedAt: event.timestamp,
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    };
  }

  if (event.type === "tool_call_completed") {
    return {
      id: resolveToolStepId(event, "tool_call_completed"),
      type: "tool",
      title: String(event.payload.tool_name ?? "tool"),
      status: "success",
      output: String(event.payload.output ?? ""),
      time: event.timestamp,
      endedAt: event.timestamp,
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    };
  }

  if (event.type === "approval_requested") {
    const reason = String(event.payload.reason ?? "");
    const target = String(event.payload.target ?? "");
    const description =
      reason.length > 0 && target.length > 0
        ? `${reason} (${target})`
        : reason.length > 0
          ? reason
          : target;
    return {
      id: resolveToolStepId(event, "approval_requested"),
      type: "approval",
      title: String(event.payload.tool_name ?? "approval"),
      status: "waiting",
      description,
      time: event.timestamp,
      startedAt: event.timestamp,
      approvalId: String(event.payload.approval_id ?? ""),
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    };
  }

  if (event.type === "approval_resolved") {
    return {
      id: resolveToolStepId(event, "approval_resolved"),
      type: "approval",
      title: String(event.payload.tool_name ?? "approval"),
      status: "success",
      time: event.timestamp,
      endedAt: event.timestamp,
      approvalId: String(event.payload.approval_id ?? ""),
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    };
  }

  return {
    id: `error:${event.turn_id ?? "unknown"}:${event.event_seq}`,
    type: "summary",
    title: "error",
    status: "failed",
    output: String(event.payload.message ?? "turn failed"),
    time: event.timestamp,
    meta: event.payload
  };
}

function resolveToolStepId(event: StreamEvent, prefix: string): string {
  if (event.type === "approval_requested" || event.type === "approval_resolved") {
    const approvalId = String(event.payload.approval_id ?? "");
    if (approvalId.length > 0) {
      return approvalId;
    }
    return `${prefix}:approval:${event.turn_id ?? "unknown"}:${event.event_seq}`;
  }
  const toolCallId = toolCallIdFromPayload(event);
  if (toolCallId.length > 0) {
    return toolCallId;
  }
  return `${prefix}:${event.turn_id ?? "unknown"}:${event.event_seq}`;
}

function toolCallIdFromPayload(event: StreamEvent): string {
  return String(event.payload.tool_call_id ?? "");
}

function withDuration(step: ToolStep): ToolStep {
  const durationSec = computeDurationSec(step.startedAt, step.endedAt);
  if (durationSec === undefined) {
    return step;
  }
  return {
    ...step,
    durationSec
  };
}

function computeDurationSec(startedAt?: string, endedAt?: string): number | undefined {
  const startMs = parseIsoToMs(startedAt);
  const endMs = parseIsoToMs(endedAt);
  if (startMs === undefined || endMs === undefined || endMs < startMs) {
    return undefined;
  }
  return Math.floor((endMs - startMs) / 1000);
}

function parseIsoToMs(value?: string): number | undefined {
  if (!value) {
    return undefined;
  }
  const ms = Date.parse(value);
  return Number.isNaN(ms) ? undefined : ms;
}

function mergeAssistantDraft(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string,
  isCanonicalDelta: boolean,
  event: StreamEvent
): void {
  const idx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.isDraft
  );
  if (idx >= 0) {
    if (!isCanonicalDelta && messages[idx].hasCanonicalDelta) {
      logStreamMetric("web_stream_duplicate_drop_count", {
        sessionId: event.session_id,
        turnId,
        eventType: event.type,
        eventSeq: event.event_seq
      });
      return;
    }
    messages[idx] = {
      ...messages[idx],
      content: `${messages[idx].content}${delta}`,
      hasCanonicalDelta: messages[idx].hasCanonicalDelta || isCanonicalDelta
    };
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
    kind: "text",
    role: "assistant",
    content: delta,
    timestamp,
    turnId,
    isDraft: true,
    hasCanonicalDelta: isCanonicalDelta
  });
}

function appendReasoningDelta(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string,
  eventSeq: number
): void {
  if (!delta) {
    return;
  }
  const idx = findAssistantMessageIndex(messages, turnId);
  if (idx < 0) {
    const block: ReasoningBlock = {
      blockId: `reasoning:${turnId}:${eventSeq}`,
      turnId,
      content: delta,
      collapsed: true,
      startedAt: timestamp,
      closed: false
    };
    messages.push({
      id: crypto.randomUUID(),
      kind: "text",
      role: "assistant",
      content: "",
      timestamp,
      turnId,
      isDraft: true,
      reasoningBlocks: [block]
    });
    return;
  }
  const message = messages[idx];
  const blocks = [...(message.reasoningBlocks ?? [])];
  const openIdx = findLastOpenReasoningBlockIndex(blocks);
  if (openIdx >= 0) {
    blocks[openIdx] = {
      ...blocks[openIdx],
      content: `${blocks[openIdx].content}${delta}`
    };
  } else {
    blocks.push({
      blockId: `reasoning:${turnId}:${eventSeq}`,
      turnId,
      content: delta,
      collapsed: true,
      startedAt: timestamp,
      closed: false
    });
  }
  messages[idx] = {
    ...message,
    timestamp,
    reasoningBlocks: blocks
  };
}

function closeOpenReasoningBlock(messages: ChatMessage[], turnId: string): void {
  const idx = findAssistantMessageIndex(messages, turnId);
  if (idx < 0) {
    return;
  }
  const message = messages[idx];
  if (!message.reasoningBlocks || message.reasoningBlocks.length === 0) {
    return;
  }
  const blocks = [...message.reasoningBlocks];
  const openIdx = findLastOpenReasoningBlockIndex(blocks);
  if (openIdx < 0) {
    return;
  }
  blocks[openIdx] = {
    ...blocks[openIdx],
    closed: true
  };
  messages[idx] = {
    ...message,
    reasoningBlocks: blocks
  };
}

function findLastOpenReasoningBlockIndex(blocks: ReasoningBlock[]): number {
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    if (!blocks[i].closed) {
      return i;
    }
  }
  return -1;
}

function findAssistantMessageIndex(messages: ChatMessage[], turnId: string): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.turnId === turnId && message.kind === "text" && message.role === "assistant") {
      return i;
    }
  }
  return -1;
}

function batchStatusFromPayload(payload: Record<string, unknown>): ToolStep["status"] {
  const failed = Number(payload.failed ?? 0);
  return failed > 0 ? "failed" : "success";
}

function upsertToolBatchSummary(
  messages: ChatMessage[],
  event: StreamEvent,
  status: ToolStep["status"]
): void {
  const turnId = event.turn_id;
  const summaryId = `tool_batch:${turnId ?? "unknown"}`;
  const output =
    event.type === "tool_batch_completed"
      ? `total=${String(event.payload.total ?? 0)}, succeeded=${String(event.payload.succeeded ?? 0)}, failed=${String(event.payload.failed ?? 0)}`
      : JSON.stringify(event.payload.tool_calls ?? []);
  const idx = messages.findIndex(
    (message) =>
      message.kind === "tool_steps" &&
      message.turnId === turnId &&
      message.toolSteps?.[0]?.id === summaryId
  );
  const step: ToolStep = {
    id: summaryId,
    type: "summary",
    title: "tool_batch",
    status,
    output,
    time: event.timestamp,
    startedAt: event.type === "tool_calls_proposed" ? event.timestamp : undefined,
    endedAt: event.type === "tool_batch_completed" ? event.timestamp : undefined,
    meta: event.payload
  };
  if (idx >= 0) {
    messages[idx] = {
      ...messages[idx],
      timestamp: event.timestamp,
      toolSteps: [withDuration({ ...(messages[idx].toolSteps?.[0] ?? step), ...step })]
    };
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
    kind: "tool_steps",
    role: "assistant",
    content: "",
    timestamp: event.timestamp,
    turnId,
    isDraft: false,
    toolSteps: [step]
  });
}

function finalizeAssistantMessageFallback(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string,
  event: StreamEvent
): void {
  const draftIdx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant" && message.isDraft
  );
  if (draftIdx >= 0) {
    if (content && content !== messages[draftIdx].content) {
      logStreamMetric("web_stream_content_mismatch_count", {
        sessionId: event.session_id,
        turnId,
        eventType: event.type,
        eventSeq: event.event_seq
      });
    }
    return;
  }
  const existingIdx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant"
  );
  if (existingIdx >= 0) {
    return;
  }
  if (!content) {
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
    kind: "text",
    role: "assistant",
    content,
    timestamp,
    turnId,
    isDraft: false
  });
}

function sealAssistantMessage(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string,
  event: StreamEvent
): void {
  const draftIdx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant" && message.isDraft
  );
  if (draftIdx >= 0) {
    if (content && content !== messages[draftIdx].content) {
      logStreamMetric("web_stream_content_mismatch_count", {
        sessionId: event.session_id,
        turnId,
        eventType: event.type,
        eventSeq: event.event_seq
      });
    }
    messages[draftIdx] = {
      ...messages[draftIdx],
      isDraft: false,
      timestamp
    };
    return;
  }

  const existingIdx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant"
  );
  if (existingIdx >= 0) {
    return;
  }

  if (!content) {
    logStreamMetric("web_stream_completed_without_draft_count", {
      sessionId: event.session_id,
      turnId,
      eventType: event.type,
      eventSeq: event.event_seq
    });
    return;
  }

  messages.push({
    id: crypto.randomUUID(),
    kind: "text",
    role: "assistant",
    content,
    timestamp,
    turnId,
    isDraft: false
  });
}

function markTurnDraftFinal(messages: ChatMessage[], turnId?: string): void {
  if (!turnId) {
    return;
  }
  for (let i = 0; i < messages.length; i += 1) {
    const message = messages[i];
    if (message.turnId === turnId && message.kind === "text" && message.isDraft) {
      messages[i] = { ...message, isDraft: false };
    }
  }
}

function isCanonicalDeltaEvent(event: StreamEvent): boolean {
  if (event.type !== "response_text_delta") {
    return false;
  }
  return Object.prototype.hasOwnProperty.call(event.payload, "stream_source");
}

function logStreamMetric(name: string, payload: Record<string, unknown>): void {
  console.debug("[stream_metrics]", name, payload);
}
