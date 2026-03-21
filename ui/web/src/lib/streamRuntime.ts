import type { ChatMessage, ChatSession, SessionStreamingState } from "../types/chat";
import type { StreamEvent } from "../types/gateway";

const STREAM_DEBUG_ENABLED = resolveStreamDebugEnabled();

type TextStreamEventType =
  | "response_started"
  | "response_text_delta"
  | "response_completed"
  | "response_error";

export function isTextStreamEvent(event: StreamEvent): event is StreamEvent & { type: TextStreamEventType } {
  return (
    event.type === "response_started" ||
    event.type === "response_text_delta" ||
    event.type === "response_completed" ||
    event.type === "response_error"
  );
}

export function applyTextStreamEvent(session: ChatSession, event: StreamEvent): ChatSession {
  if (!isTextStreamEvent(event)) {
    return session;
  }
  if (!shouldAcceptEvent(session, event)) {
    if (STREAM_DEBUG_ENABLED) {
      console.debug("[stream_debug][runtime][drop_seq_gate]", {
        sessionId: event.session_id,
        turnId: event.turn_id,
        eventType: event.type,
        eventSeq: event.event_seq,
        sessionLastEventSeq: session.lastEventSeq
      });
    }
    return session;
  }

  const next: ChatSession = {
    ...session,
    lastEventSeq: Math.max(session.lastEventSeq, event.event_seq),
    pendingApprovals: [...session.pendingApprovals],
    messages: [...session.messages]
  };

  if (event.type === "response_started") {
    if (STREAM_DEBUG_ENABLED) {
      console.debug("[stream_debug][runtime][response_started]", {
        sessionId: event.session_id,
        turnId: event.turn_id,
        eventSeq: event.event_seq
      });
    }
    const started = onResponseStarted(next, event);
    return {
      ...started,
      streaming: nextStreaming(started.streaming, event, true)
    };
  }

  if (event.type === "response_text_delta") {
    if (STREAM_DEBUG_ENABLED) {
      console.debug("[stream_debug][runtime][delta]", {
        sessionId: event.session_id,
        turnId: event.turn_id,
        eventType: event.type,
        eventSeq: event.event_seq,
        delta: String(event.payload.content_delta ?? ""),
        deltaLen: String(event.payload.content_delta ?? "").length,
        beforeLen: session.streaming?.content.length ?? 0
      });
    }
    const withDelta = onResponseTextDelta(next, event);
    return {
      ...withDelta,
      streaming: nextStreaming(withDelta.streaming, event, true)
    };
  }

  if (event.type === "response_completed") {
    if (STREAM_DEBUG_ENABLED) {
      console.debug("[stream_debug][runtime][complete]", {
        sessionId: event.session_id,
        turnId: event.turn_id,
        eventType: event.type,
        eventSeq: event.event_seq,
        completedLen: String(event.payload.content ?? "").length,
        beforeLen: session.streaming?.content.length ?? 0
      });
    }
    const completed = onResponseCompleted(next, event);
    return {
      ...completed,
      streaming: nextStreaming(completed.streaming, event, false)
    };
  }

  const failed = onResponseError(next, event);
  return {
    ...failed,
    streaming: nextStreaming(failed.streaming, event, false)
  };
}

function shouldAcceptEvent(session: ChatSession, event: StreamEvent): boolean {
  if (event.event_seq > session.lastEventSeq) {
    return true;
  }
  return looksLikeSequenceReset(session, event);
}

function looksLikeSequenceReset(session: ChatSession, event: StreamEvent): boolean {
  if (session.lastEventSeq <= 0) {
    return false;
  }
  if (event.event_seq === 1) {
    return true;
  }
  if (event.turn_seq === 1 && event.type === "response_started") {
    return true;
  }
  return false;
}

function onResponseStarted(session: ChatSession, event: StreamEvent): ChatSession {
  const turnId = event.turn_id;
  const message = ensureDraftMessage(session.messages, turnId, event.timestamp);
  return {
    ...session,
    turnPhase: "streaming",
    streaming: {
      turnId,
      assistantMessageId: message?.id,
      content: message?.content ?? "",
      lastEventSeq: event.event_seq,
      active: true
    }
  };
}

function onResponseTextDelta(session: ChatSession, event: StreamEvent): ChatSession {
  const turnId = event.turn_id;
  const delta = String(event.payload.content_delta ?? "");
  const draft = ensureDraftMessage(session.messages, turnId, event.timestamp);
  const updatedContent = draft && delta.length > 0 ? `${draft.content}${delta}` : draft?.content ?? "";
  if (draft && (delta.length > 0 || draft.isDraft !== true)) {
    session.messages[draft.index] = {
      ...draft,
      content: updatedContent,
      timestamp: event.timestamp,
      isDraft: true
    };
  }
  return {
    ...session,
    turnPhase: "streaming",
    streaming: {
      turnId,
      assistantMessageId: draft?.id,
      content: updatedContent,
      lastEventSeq: event.event_seq,
      active: true
    }
  };
}

function onResponseCompleted(session: ChatSession, event: StreamEvent): ChatSession {
  const turnId = event.turn_id;
  const content = String(event.payload.content ?? "");
  const message = findDraftOrAssistantMessage(session.messages, turnId);
  if (message) {
    const resolvedContent = resolveCompletedContent(message.content, content);
    session.messages[message.index] = {
      ...message,
      content: resolvedContent,
      isDraft: false,
      timestamp: event.timestamp
    };
  } else if (content) {
    session.messages.push({
      id: crypto.randomUUID(),
      kind: "text",
      role: "assistant",
      content,
      timestamp: event.timestamp,
      turnId,
      isDraft: false
    });
  }
  const completedMessage = findDraftOrAssistantMessage(session.messages, turnId);
  if (content && completedMessage && content !== completedMessage.content) {
    console.debug("[stream_runtime] completion_content_mismatch", {
      sessionId: event.session_id,
      turnId,
      eventType: event.type,
      eventSeq: event.event_seq,
      currentLength: completedMessage.content.length,
      completedLength: content.length
    });
  }
  return {
    ...session,
    turnPhase: "completed",
    streaming: {
      turnId,
      assistantMessageId: completedMessage?.id,
      content: completedMessage?.content ?? content,
      lastEventSeq: event.event_seq,
      active: false
    }
  };
}

function onResponseError(session: ChatSession, event: StreamEvent): ChatSession {
  const turnId = event.turn_id;
  const message = String(event.payload.message ?? "turn failed");
  session.messages.push({
    id: crypto.randomUUID(),
    kind: "text",
    role: "error",
    content: message,
    turnId,
    timestamp: event.timestamp
  });
  return {
    ...session,
    turnPhase: "failed",
    streaming: {
      turnId,
      assistantMessageId: session.streaming?.assistantMessageId,
      content: session.streaming?.content ?? "",
      lastEventSeq: event.event_seq,
      active: false
    }
  };
}

function nextStreaming(
  current: SessionStreamingState | undefined,
  event: StreamEvent,
  active: boolean
): SessionStreamingState | undefined {
  if (!current) {
    if (!event.turn_id) {
      return undefined;
    }
    return {
      turnId: event.turn_id,
      content: "",
      lastEventSeq: event.event_seq,
      active
    };
  }
  return {
    ...current,
    turnId: event.turn_id ?? current.turnId,
    lastEventSeq: Math.max(current.lastEventSeq, event.event_seq),
    active
  };
}

function ensureDraftMessage(
  messages: ChatMessage[],
  turnId: string | undefined,
  timestamp: string
): (ChatMessage & { index: number }) | undefined {
  const existing = findDraftOrAssistantMessage(messages, turnId);
  if (existing) {
    const next = {
      ...existing,
      isDraft: true
    };
    messages[existing.index] = next;
    return {
      ...next,
      index: existing.index
    };
  }
  const created: ChatMessage = {
    id: crypto.randomUUID(),
    kind: "text",
    role: "assistant",
    content: "",
    timestamp,
    turnId,
    isDraft: true
  };
  messages.push(created);
  return {
    ...created,
    index: messages.length - 1
  };
}

function findDraftOrAssistantMessage(
  messages: ChatMessage[],
  turnId: string | undefined
): (ChatMessage & { index: number }) | undefined {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.kind !== "text" || message.role !== "assistant") {
      continue;
    }
    if (message.turnId === turnId) {
      return {
        ...message,
        index: i
      };
    }
  }
  return undefined;
}

function resolveCompletedContent(current: string, completed: string): string {
  if (!completed) {
    return current;
  }
  if (!current) {
    return completed;
  }
  // Keep streaming-first behavior but repair the common case where tail deltas were missed.
  if (completed.startsWith(current)) {
    return completed;
  }
  return current;
}

function resolveStreamDebugEnabled(): boolean {
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
          VITE_OPENJAX_WEB_STREAM_DEBUG?: string | boolean;
        })
      : {};
  const raw = String(
    globals.OPENJAX_WEB_STREAM_DEBUG ??
      globals.VITE_OPENJAX_WEB_STREAM_DEBUG ??
      "0"
  )
    .trim()
    .toLowerCase();
  return !(raw === "0" || raw === "off" || raw === "false" || raw === "disabled");
}
