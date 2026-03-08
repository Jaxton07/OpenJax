import type { ChatMessage, ChatSession } from "../types/chat";
import type { StreamEvent } from "../types/gateway";

export function applyStreamEvent(session: ChatSession, event: StreamEvent): ChatSession {
  if (event.event_seq <= session.lastEventSeq) {
    return session;
  }

  const next: ChatSession = {
    ...session,
    lastEventSeq: event.event_seq,
    pendingApprovals: [...session.pendingApprovals],
    messages: [...session.messages]
  };

  const turnId = event.turn_id;
  if (event.type === "turn_started") {
    next.turnPhase = "streaming";
  }

  if (event.type === "assistant_delta" && turnId) {
    const contentDelta = String(event.payload.content_delta ?? "");
    mergeAssistantDraft(next.messages, turnId, contentDelta, event.timestamp);
  }

  if (event.type === "assistant_message" && turnId) {
    const content = String(event.payload.content ?? "");
    finalizeAssistantMessage(next.messages, turnId, content, event.timestamp);
  }

  if (event.type === "tool_call_started") {
    const toolName = String(event.payload.tool_name ?? "tool");
    const target = String(event.payload.target ?? "");
    next.messages.push({
      id: crypto.randomUUID(),
      role: "tool",
      content: `[${toolName}] ${target}`,
      turnId,
      timestamp: event.timestamp
    });
  }

  if (event.type === "tool_call_completed") {
    const toolName = String(event.payload.tool_name ?? "tool");
    const output = String(event.payload.output ?? "");
    next.messages.push({
      id: crypto.randomUUID(),
      role: "tool",
      content: `[${toolName}] 完成\n${output}`,
      turnId,
      timestamp: event.timestamp
    });
  }

  if (event.type === "approval_requested") {
    next.pendingApprovals.push({
      approvalId: String(event.payload.approval_id ?? ""),
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

  if (event.type === "turn_completed") {
    next.turnPhase = "completed";
    markTurnDraftFinal(next.messages, turnId);
  }

  if (event.type === "error") {
    next.turnPhase = "failed";
    const message = String(event.payload.message ?? "turn failed");
    next.messages.push({
      id: crypto.randomUUID(),
      role: "error",
      content: message,
      turnId,
      timestamp: event.timestamp
    });
  }

  return next;
}

function mergeAssistantDraft(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string
): void {
  const idx = messages.findIndex((message) => message.turnId === turnId && message.isDraft);
  if (idx >= 0) {
    messages[idx] = {
      ...messages[idx],
      content: `${messages[idx].content}${delta}`
    };
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
    role: "assistant",
    content: delta,
    timestamp,
    turnId,
    isDraft: true
  });
}

function finalizeAssistantMessage(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string
): void {
  const idx = messages.findIndex((message) => message.turnId === turnId && message.role === "assistant");
  if (idx >= 0) {
    messages[idx] = {
      ...messages[idx],
      content,
      timestamp,
      isDraft: false
    };
    return;
  }
  messages.push({
    id: crypto.randomUUID(),
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
    if (message.turnId === turnId && message.isDraft) {
      messages[i] = { ...message, isDraft: false };
    }
  }
}
