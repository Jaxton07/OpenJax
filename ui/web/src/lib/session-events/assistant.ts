import type { ChatMessage, ChatSession, ReasoningBlock } from "../../types/chat";
import type { StreamEvent } from "../../types/gateway";

export function shouldCloseReasoningOnEvent(event: StreamEvent): boolean {
  return (
    event.type === "response_text_delta" ||
    event.type === "assistant_message" ||
    event.type === "tool_calls_proposed" ||
    event.type === "tool_call_started" ||
    event.type === "tool_call_ready" ||
    event.type === "tool_call_completed" ||
    event.type === "approval_requested" ||
    event.type === "approval_resolved" ||
    event.type === "error" ||
    event.type === "response_completed" ||
    event.type === "response_error" ||
    event.type === "turn_completed" ||
    event.type === "turn_interrupted"
  );
}

export function applyResponseStartedSession(
  session: ChatSession,
  event: StreamEvent
): { session: ChatSession; messageId?: string; content: string } {
  const turnId = event.turn_id;
  if (!turnId) {
    return { session, content: "" };
  }
  const responseSegmentId = responseSegmentIdFromEvent(event);

  const existingIndex = findAssistantMessageIndex(session.messages, turnId, responseSegmentId);
  const messages = [...session.messages];
  let messageId: string;
  let content = "";
  if (existingIndex >= 0) {
    const existing = messages[existingIndex];
    messageId = existing.id;
    content = existing.content;
    messages[existingIndex] = {
      ...existing,
      isDraft: true,
      responseSegmentId: responseSegmentId ?? existing.responseSegmentId,
      timestamp: event.timestamp,
      startEventSeq: existing.startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(existing.lastEventSeq ?? event.event_seq, event.event_seq)
    };
  } else {
    messageId = buildAssistantDraftId(turnId, responseSegmentId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: "",
      timestamp: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      turnId,
      responseSegmentId,
      isDraft: true
    });
  }

  return {
    session: {
      ...session,
      turnPhase: "streaming",
      lastEventSeq: Math.max(session.lastEventSeq, event.event_seq),
      messages,
      streaming: {
        turnId,
        assistantMessageId: messageId,
        content,
        lastEventSeq: event.event_seq,
        active: true
      }
    },
    messageId,
    content
  };
}

export function applyResponseCompletedSession(
  session: ChatSession,
  event: StreamEvent,
  finalizedContent: string
): ChatSession {
  const turnId = event.turn_id;
  if (!turnId) {
    return session;
  }
  const responseSegmentId = responseSegmentIdFromEvent(event);

  let nextContent = finalizedContent;
  const messages = [...session.messages];
  const index = findAssistantMessageIndex(messages, turnId, responseSegmentId);
  let messageId: string;
  if (index >= 0) {
    const message = messages[index];
    if (event.type === "assistant_message" && message.content.length > nextContent.length) {
      // Guard against late, shorter assistant_message payload overriding a fuller finalized body.
      nextContent = message.content;
    }
    messageId = message.id;
    messages[index] = {
      ...message,
      content: nextContent,
      isDraft: false,
      responseSegmentId: responseSegmentId ?? message.responseSegmentId,
      timestamp: event.timestamp,
      startEventSeq: message.startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(message.lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: message.textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(message.textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(message.textEndEventSeq ?? event.event_seq, event.event_seq)
    };
  } else {
    messageId = buildAssistantDraftId(turnId, responseSegmentId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: nextContent,
      timestamp: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      textStartEventSeq: event.event_seq,
      textLastEventSeq: event.event_seq,
      textEndEventSeq: event.event_seq,
      turnId,
      responseSegmentId,
      isDraft: false
    });
  }

  return {
    ...session,
    turnPhase: "completed",
    lastEventSeq: Math.max(session.lastEventSeq, event.event_seq),
    messages,
    streaming: {
      turnId,
      assistantMessageId: messageId,
      content: nextContent,
      lastEventSeq: event.event_seq,
      active: false
    }
  };
}

export function mergeAssistantDraft(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string,
  isCanonicalDelta: boolean,
  event: StreamEvent
): void {
  const responseSegmentId = responseSegmentIdFromEvent(event);
  const idx = findAssistantDraftIndex(messages, turnId, responseSegmentId);
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
      hasCanonicalDelta: messages[idx].hasCanonicalDelta || isCanonicalDelta,
      responseSegmentId: responseSegmentId ?? messages[idx].responseSegmentId,
      startEventSeq: messages[idx].startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(messages[idx].lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: messages[idx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[idx].textLastEventSeq ?? event.event_seq, event.event_seq)
    };
    return;
  }
  messages.push({
    id: buildAssistantDraftId(turnId, responseSegmentId),
    kind: "text",
    role: "assistant",
    content: delta,
    timestamp,
    startEventSeq: event.event_seq,
    lastEventSeq: event.event_seq,
    textStartEventSeq: event.event_seq,
    textLastEventSeq: event.event_seq,
    turnId,
    responseSegmentId,
    isDraft: true,
    hasCanonicalDelta: isCanonicalDelta
  });
}

export function appendReasoningDelta(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string,
  eventSeq: number,
  reasoningSegmentId?: string
): void {
  if (!delta) {
    return;
  }
  const idx = findAssistantMessageIndex(messages, turnId);
  const targetIdx =
    reasoningSegmentId && reasoningSegmentId.length > 0
      ? findAssistantMessageIndexByReasoningSegment(messages, turnId, reasoningSegmentId) ?? idx
      : idx;
  if (targetIdx < 0) {
    const block: ReasoningBlock = {
      blockId: buildReasoningBlockId(turnId, eventSeq, reasoningSegmentId),
      turnId,
      reasoningSegmentId,
      content: delta,
      collapsed: true,
      startedAt: timestamp,
      closed: false,
      startEventSeq: eventSeq,
      lastEventSeq: eventSeq
    };
    messages.push({
      id: crypto.randomUUID(),
      kind: "text",
      role: "assistant",
      content: "",
      timestamp,
      startEventSeq: eventSeq,
      lastEventSeq: eventSeq,
      turnId,
      isDraft: true,
      reasoningBlocks: [block]
    });
    return;
  }
  const message = messages[targetIdx];
  const blocks = [...(message.reasoningBlocks ?? [])];
  const hasReasoningSegmentId = Boolean(reasoningSegmentId && reasoningSegmentId.length > 0);
  const segmentedIdx = hasReasoningSegmentId
    ? blocks.findIndex((block) => block.reasoningSegmentId === reasoningSegmentId)
    : -1;
  if (segmentedIdx >= 0) {
    blocks[segmentedIdx] = {
      ...blocks[segmentedIdx],
      content: `${blocks[segmentedIdx].content}${delta}`,
      reasoningSegmentId: reasoningSegmentId ?? blocks[segmentedIdx].reasoningSegmentId,
      closed: false,
      endEventSeq: undefined,
      endedAt: undefined,
      lastEventSeq: Math.max(blocks[segmentedIdx].lastEventSeq ?? eventSeq, eventSeq)
    };
  } else if (hasReasoningSegmentId) {
    for (let i = 0; i < blocks.length; i += 1) {
      if (!blocks[i].closed) {
        blocks[i] = {
          ...blocks[i],
          closed: true,
          endEventSeq: blocks[i].lastEventSeq ?? eventSeq,
          endedAt: timestamp
        };
      }
    }
    blocks.push({
      blockId: buildReasoningBlockId(turnId, eventSeq, reasoningSegmentId),
      turnId,
      reasoningSegmentId,
      content: delta,
      collapsed: true,
      startedAt: timestamp,
      closed: false,
      startEventSeq: eventSeq,
      lastEventSeq: eventSeq
    });
  } else {
    const openIdx = findLastOpenReasoningBlockIndex(blocks);
    if (openIdx >= 0) {
      blocks[openIdx] = {
        ...blocks[openIdx],
        content: `${blocks[openIdx].content}${delta}`,
        closed: false,
        endEventSeq: undefined,
        endedAt: undefined,
        lastEventSeq: Math.max(blocks[openIdx].lastEventSeq ?? eventSeq, eventSeq)
      };
    } else {
      blocks.push({
        blockId: buildReasoningBlockId(turnId, eventSeq, undefined),
        turnId,
        content: delta,
        collapsed: true,
        startedAt: timestamp,
        closed: false,
        startEventSeq: eventSeq,
        lastEventSeq: eventSeq
      });
    }
  }
  messages[targetIdx] = {
    ...message,
    timestamp,
    startEventSeq: message.startEventSeq ?? eventSeq,
    lastEventSeq: Math.max(message.lastEventSeq ?? eventSeq, eventSeq),
    reasoningBlocks: blocks
  };
}

export function closeOpenReasoningBlock(messages: ChatMessage[], turnId: string, eventSeq?: number, timestamp?: string): void {
  for (let i = 0; i < messages.length; i += 1) {
    const message = messages[i];
    if (message.turnId !== turnId || message.kind !== "text" || message.role !== "assistant") {
      continue;
    }
    if (!message.reasoningBlocks || message.reasoningBlocks.length === 0) {
      continue;
    }
    const blocks = [...message.reasoningBlocks];
    let changed = false;
    for (let j = 0; j < blocks.length; j += 1) {
      if (!blocks[j].closed) {
        blocks[j] = {
          ...blocks[j],
          closed: true,
          endEventSeq: blocks[j].lastEventSeq ?? eventSeq,
          endedAt: timestamp ?? new Date().toISOString()
        };
        changed = true;
      }
    }
    if (changed) {
      messages[i] = {
        ...message,
        reasoningBlocks: blocks
      };
    }
  }
}

export function closeOpenReasoningBlockInSession(
  session: ChatSession,
  turnId?: string,
  eventSeq?: number,
  timestamp?: string
): ChatSession {
  if (!turnId) {
    return session;
  }
  const messages = [...session.messages];
  closeOpenReasoningBlock(messages, turnId, eventSeq, timestamp);
  return { ...session, messages };
}

export function finalizeAssistantMessageFallback(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string,
  event: StreamEvent
): void {
  // Legacy compat: assistant_message can seed draft text, but it should not finalize the turn.
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
      textStartEventSeq: messages[draftIdx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[draftIdx].textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(messages[draftIdx].textEndEventSeq ?? event.event_seq, event.event_seq)
    };
    return;
  }
  const existingIdx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant"
  );
  if (existingIdx >= 0) {
    messages[existingIdx] = {
      ...messages[existingIdx],
      textStartEventSeq: messages[existingIdx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[existingIdx].textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(messages[existingIdx].textEndEventSeq ?? event.event_seq, event.event_seq)
    };
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
    startEventSeq: event.event_seq,
    lastEventSeq: event.event_seq,
    textStartEventSeq: event.event_seq,
    textLastEventSeq: event.event_seq,
    textEndEventSeq: event.event_seq,
    turnId,
    isDraft: true
  });
}

export function sealAssistantMessage(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string,
  event: StreamEvent
): void {
  const responseSegmentId = responseSegmentIdFromEvent(event);
  const draftIdx = findAssistantDraftIndex(messages, turnId, responseSegmentId);
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
      content: content || messages[draftIdx].content,
      isDraft: false,
      responseSegmentId: responseSegmentId ?? messages[draftIdx].responseSegmentId,
      timestamp,
      lastEventSeq: Math.max(messages[draftIdx].lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: messages[draftIdx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[draftIdx].textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(messages[draftIdx].textEndEventSeq ?? event.event_seq, event.event_seq)
    };
    return;
  }

  const existingIdx = messages.findIndex(
    (message) =>
      message.turnId === turnId &&
      message.kind === "text" &&
      message.role === "assistant" &&
      (!responseSegmentId || message.responseSegmentId === responseSegmentId)
  );
  if (existingIdx >= 0) {
    messages[existingIdx] = {
      ...messages[existingIdx],
      content: content || messages[existingIdx].content,
      isDraft: false,
      responseSegmentId: responseSegmentId ?? messages[existingIdx].responseSegmentId,
      timestamp,
      lastEventSeq: Math.max(messages[existingIdx].lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: messages[existingIdx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[existingIdx].textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(messages[existingIdx].textEndEventSeq ?? event.event_seq, event.event_seq)
    };
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
    id: buildAssistantDraftId(turnId, responseSegmentId),
    kind: "text",
    role: "assistant",
    content,
    timestamp,
    startEventSeq: event.event_seq,
    lastEventSeq: event.event_seq,
    textStartEventSeq: event.event_seq,
    textLastEventSeq: event.event_seq,
    textEndEventSeq: event.event_seq,
    turnId,
    responseSegmentId,
    isDraft: false
  });
}

export function touchAssistantTextSeqInSession(
  session: ChatSession,
  turnId?: string,
  eventSeq?: number,
  timestamp?: string
): ChatSession {
  if (!turnId || eventSeq === undefined) {
    return session;
  }
  const idx = findAssistantMessageIndex(session.messages, turnId);
  if (idx < 0) {
    return session;
  }
  const message = session.messages[idx];
  const messages = [...session.messages];
  messages[idx] = {
    ...message,
    timestamp: timestamp ?? message.timestamp,
    textStartEventSeq: message.textStartEventSeq ?? eventSeq,
    textLastEventSeq: Math.max(message.textLastEventSeq ?? eventSeq, eventSeq)
  };
  return {
    ...session,
    messages
  };
}

export function markTurnDraftFinal(messages: ChatMessage[], turnId?: string): void {
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

export function findAssistantMessageIndex(messages: ChatMessage[], turnId: string): number;
export function findAssistantMessageIndex(
  messages: ChatMessage[],
  turnId: string,
  responseSegmentId?: string
): number;
export function findAssistantMessageIndex(
  messages: ChatMessage[],
  turnId: string,
  responseSegmentId?: string
): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (
      message.turnId === turnId &&
      message.kind === "text" &&
      message.role === "assistant" &&
      (!responseSegmentId || message.responseSegmentId === responseSegmentId)
    ) {
      return i;
    }
  }
  return -1;
}

export function isCanonicalDeltaEvent(event: StreamEvent): boolean {
  if (event.type !== "response_text_delta") {
    return false;
  }
  return Object.prototype.hasOwnProperty.call(event.payload, "stream_source");
}

function buildAssistantDraftId(turnId: string, responseSegmentId?: string): string {
  if (!responseSegmentId) {
    return `assistant_draft_${turnId}`;
  }
  return `assistant_${turnId}_${responseSegmentId}`;
}

function buildReasoningBlockId(turnId: string, eventSeq: number, reasoningSegmentId?: string): string {
  if (!reasoningSegmentId) {
    return `reasoning:${turnId}:${eventSeq}`;
  }
  return `reasoning:${turnId}:${reasoningSegmentId}`;
}

function responseSegmentIdFromEvent(event: StreamEvent): string | undefined {
  const value = event.payload.response_segment_id;
  if (typeof value === "string" && value.length > 0) {
    return value;
  }
  return undefined;
}

function findAssistantDraftIndex(
  messages: ChatMessage[],
  turnId: string,
  responseSegmentId?: string
): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (
      message.turnId === turnId &&
      message.kind === "text" &&
      message.role === "assistant" &&
      message.isDraft &&
      (!responseSegmentId || message.responseSegmentId === responseSegmentId)
    ) {
      return i;
    }
  }
  if (!responseSegmentId) {
    return -1;
  }
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (
      message.turnId === turnId &&
      message.kind === "text" &&
      message.role === "assistant" &&
      message.responseSegmentId === responseSegmentId
    ) {
      return i;
    }
  }
  return -1;
}

function findAssistantMessageIndexByReasoningSegment(
  messages: ChatMessage[],
  turnId: string,
  reasoningSegmentId: string
): number | undefined {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.turnId !== turnId || message.kind !== "text" || message.role !== "assistant") {
      continue;
    }
    if ((message.reasoningBlocks ?? []).some((block) => block.reasoningSegmentId === reasoningSegmentId)) {
      return i;
    }
  }
  return undefined;
}

function findLastOpenReasoningBlockIndex(blocks: ReasoningBlock[]): number {
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    if (!blocks[i].closed) {
      return i;
    }
  }
  return -1;
}

function logStreamMetric(name: string, payload: Record<string, unknown>): void {
  console.debug("[stream_metrics]", name, payload);
}
