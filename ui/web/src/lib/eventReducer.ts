import type { ChatMessage, ChatSession, ToolStep } from "../types/chat";
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

  if (isToolStepEvent(event)) {
    upsertToolStepMessage(next.messages, event);
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
      kind: "text",
      role: "error",
      content: message,
      turnId,
      timestamp: event.timestamp
    });
  }

  return next;
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

function upsertToolStepMessage(messages: ChatMessage[], event: StreamEvent): void {
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
    if (event.type === "tool_call_completed") {
      return findFallbackToolMatch(messages, event);
    }
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
  // Some backends emit partial identifiers (e.g. completed has tool_call_id but started does not).
  // For completed events, fallback to turn + running tool name to avoid split cards.
  if (event.type === "tool_call_completed") {
    return findFallbackToolMatch(messages, event);
  }
  return -1;
}

function mergeToolStep(previous: ToolStep | undefined, next: ToolStep): ToolStep {
  if (!previous) {
    return next;
  }
  // Keep existing tool identity when approval events update the same tool call card.
  if (previous.type === "tool" && next.type === "approval") {
    return {
      ...previous,
      status: next.status,
      time: next.time,
      description: next.description ?? previous.description,
      approvalId: next.approvalId ?? previous.approvalId,
      toolCallId: next.toolCallId ?? previous.toolCallId,
      meta: next.meta
    };
  }
  return { ...previous, ...next };
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

function findFallbackToolMatch(messages: ChatMessage[], event: StreamEvent): number {
  const toolName = String(event.payload.tool_name ?? "");
  if (toolName.length === 0) {
    return -1;
  }
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    const step = message.toolSteps?.[0];
    if (message.kind !== "tool_steps" || message.turnId !== event.turn_id || !step) {
      continue;
    }
    if (step.type !== "tool" || step.status !== "running") {
      continue;
    }
    if (String(step.title ?? "") !== toolName) {
      continue;
    }
    if (String(step.toolCallId ?? "").length > 0) {
      continue;
    }
    return i;
  }
  return -1;
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

function mergeAssistantDraft(
  messages: ChatMessage[],
  turnId: string,
  delta: string,
  timestamp: string
): void {
  const idx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.isDraft
  );
  if (idx >= 0) {
    messages[idx] = {
      ...messages[idx],
      content: `${messages[idx].content}${delta}`
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
    isDraft: true
  });
}

function finalizeAssistantMessage(
  messages: ChatMessage[],
  turnId: string,
  content: string,
  timestamp: string
): void {
  const idx = messages.findIndex(
    (message) => message.turnId === turnId && message.kind === "text" && message.role === "assistant"
  );
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
