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
      timestamp: event.timestamp
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: "",
      timestamp: event.timestamp,
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
      timestamp: event.timestamp
    };
  } else {
    messageId = buildAssistantDraftId(turnId);
    messages.push({
      id: messageId,
      kind: "text",
      role: "assistant",
      content: nextContent,
      timestamp: event.timestamp,
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

export function closeOpenReasoningBlock(messages: ChatMessage[], turnId: string): void {
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

export function closeOpenReasoningBlockInSession(session: ChatSession, turnId?: string): ChatSession {
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
    closed: true
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
  if (existingIdx >= 0 || !content) {
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
