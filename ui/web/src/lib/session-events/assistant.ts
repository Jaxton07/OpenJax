import type { ChatMessage, ChatSession, ReasoningBlock } from "../../types/chat";
import type { StreamEvent } from "../../types/gateway";

export function shouldCloseReasoningOnEvent(event: StreamEvent): boolean {
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

export function applyResponseStartedSession(
  session: ChatSession,
  event: StreamEvent
): { session: ChatSession; messageId?: string; content: string } {
  const turnId = event.turn_id;
  if (!turnId) {
    return { session, content: "" };
  }

  const existingIndex = findAssistantMessageIndex(session.messages, turnId);
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
      timestamp: event.timestamp,
      startEventSeq: existing.startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(existing.lastEventSeq ?? event.event_seq, event.event_seq)
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: "",
      timestamp: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      turnId,
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

  let nextContent = finalizedContent;
  const messages = [...session.messages];
  const index = findAssistantMessageIndex(messages, turnId);
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
      timestamp: event.timestamp,
      startEventSeq: message.startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(message.lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: message.textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(message.textLastEventSeq ?? event.event_seq, event.event_seq),
      textEndEventSeq: Math.max(message.textEndEventSeq ?? event.event_seq, event.event_seq)
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
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
      hasCanonicalDelta: messages[idx].hasCanonicalDelta || isCanonicalDelta,
      startEventSeq: messages[idx].startEventSeq ?? event.event_seq,
      lastEventSeq: Math.max(messages[idx].lastEventSeq ?? event.event_seq, event.event_seq),
      textStartEventSeq: messages[idx].textStartEventSeq ?? event.event_seq,
      textLastEventSeq: Math.max(messages[idx].textLastEventSeq ?? event.event_seq, event.event_seq)
    };
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
    kind: "text",
    role: "assistant",
    content: delta,
    timestamp,
    startEventSeq: event.event_seq,
    lastEventSeq: event.event_seq,
    textStartEventSeq: event.event_seq,
    textLastEventSeq: event.event_seq,
    turnId,
    isDraft: true,
    hasCanonicalDelta: isCanonicalDelta
  });
}

export function appendReasoningDelta(
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
  const message = messages[idx];
  const blocks = [...(message.reasoningBlocks ?? [])];
  const openIdx = findLastOpenReasoningBlockIndex(blocks);
  if (openIdx >= 0) {
    blocks[openIdx] = {
      ...blocks[openIdx],
      content: `${blocks[openIdx].content}${delta}`,
      lastEventSeq: Math.max(blocks[openIdx].lastEventSeq ?? eventSeq, eventSeq)
    };
  } else {
    blocks.push({
      blockId: `reasoning:${turnId}:${eventSeq}`,
      turnId,
      content: delta,
      collapsed: true,
      startedAt: timestamp,
      closed: false,
      startEventSeq: eventSeq,
      lastEventSeq: eventSeq
    });
  }
  messages[idx] = {
    ...message,
    timestamp,
    startEventSeq: message.startEventSeq ?? eventSeq,
    lastEventSeq: Math.max(message.lastEventSeq ?? eventSeq, eventSeq),
    reasoningBlocks: blocks
  };
}

export function closeOpenReasoningBlock(messages: ChatMessage[], turnId: string, eventSeq?: number, timestamp?: string): void {
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
    closed: true,
    endEventSeq: blocks[openIdx].lastEventSeq ?? eventSeq,
    endedAt: timestamp ?? new Date().toISOString()
  };
  messages[idx] = {
    ...message,
    reasoningBlocks: blocks
  };
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
  const idx = findAssistantMessageIndex(session.messages, turnId);
  if (idx < 0) {
    return session;
  }
  const message = session.messages[idx];
  const blocks = message.reasoningBlocks;
  if (!blocks || blocks.length === 0) {
    return session;
  }
  const openIdx = findLastOpenReasoningBlockIndex(blocks);
  if (openIdx < 0) {
    return session;
  }
  const nextBlocks = [...blocks];
  nextBlocks[openIdx] = {
    ...nextBlocks[openIdx],
    closed: true,
    endEventSeq: nextBlocks[openIdx].lastEventSeq ?? eventSeq,
    endedAt: timestamp ?? new Date().toISOString()
  };
  const messages = [...session.messages];
  messages[idx] = {
    ...message,
    reasoningBlocks: nextBlocks
  };
  return {
    ...session,
    messages
  };
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
      content: content || messages[draftIdx].content,
      isDraft: false,
      timestamp,
      lastEventSeq: Math.max(messages[draftIdx].lastEventSeq ?? event.event_seq, event.event_seq),
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
      content: content || messages[existingIdx].content,
      isDraft: false,
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

export function findAssistantMessageIndex(messages: ChatMessage[], turnId: string): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.turnId === turnId && message.kind === "text" && message.role === "assistant") {
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

function buildAssistantDraftId(turnId: string): string {
  return `assistant_draft_${turnId}`;
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
