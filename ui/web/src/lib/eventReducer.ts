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

  if (event.type === "tool_call_started") {
    const toolName = String(event.payload.tool_name ?? "tool");
    const target = String(event.payload.target ?? "");
    applyToolEventToSteps(next.messages, event);
    // TODO(track-b): remove legacy tool text message path after structured toolSteps renderer ships.
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
    applyToolEventToSteps(next.messages, event);
    // TODO(track-b): remove legacy tool text message path after structured toolSteps renderer ships.
    next.messages.push({
      id: crypto.randomUUID(),
      role: "tool",
      content: `[${toolName}] 完成\n${output}`,
      turnId,
      timestamp: event.timestamp
    });
  }

  if (event.type === "approval_requested") {
    applyToolEventToSteps(next.messages, event);
    next.pendingApprovals.push({
      approvalId: String(event.payload.approval_id ?? ""),
      turnId,
      target: String(event.payload.target ?? ""),
      reason: String(event.payload.reason ?? ""),
      toolName: String(event.payload.tool_name ?? "")
    });
  }

  if (event.type === "approval_resolved") {
    applyToolEventToSteps(next.messages, event);
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
    markLatestRunningStepFailed(next.messages, event, message);
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

function applyToolEventToSteps(messages: ChatMessage[], event: StreamEvent): void {
  if (
    event.type !== "tool_call_started" &&
    event.type !== "tool_call_completed" &&
    event.type !== "approval_requested" &&
    event.type !== "approval_resolved"
  ) {
    return;
  }

  const turnId = event.turn_id;
  const toolMessage = ensureAssistantToolMessage(messages, turnId, event.timestamp);
  if (!toolMessage.toolSteps) {
    toolMessage.toolSteps = [];
  }
  const steps = toolMessage.toolSteps;

  if (event.type === "tool_call_started") {
    const stepId = resolveStepId(event);
    const toolName = String(event.payload.tool_name ?? "tool");
    upsertToolStep(steps, {
      id: stepId,
      type: "tool",
      title: toolName || "tool",
      subtitle: String(event.payload.target ?? ""),
      status: "running",
      time: event.timestamp,
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    });
    return;
  }

  if (event.type === "tool_call_completed") {
    const toolName = String(event.payload.tool_name ?? "tool");
    const output = String(event.payload.output ?? "");
    const toolCallId = toolCallIdFromPayload(event);
    const runningIdx =
      toolCallId.length > 0
        ? steps.findIndex((step) => step.id === toolCallId)
        : findLatestRunningToolStepIndex(steps);
    const stepId =
      runningIdx >= 0 ? steps[runningIdx].id : resolveStepId(event, "tool_call_completed");
    upsertToolStep(steps, {
      id: stepId,
      type: "tool",
      title: toolName || "tool",
      status: "success",
      output,
      time: event.timestamp,
      toolCallId,
      meta: event.payload
    });
    return;
  }

  if (event.type === "approval_requested") {
    const approvalId = String(event.payload.approval_id ?? "");
    const stepId = resolveStepId(event);
    const reason = String(event.payload.reason ?? "");
    const target = String(event.payload.target ?? "");
    const description =
      reason.length > 0 && target.length > 0
        ? `${reason} (${target})`
        : reason.length > 0
          ? reason
          : target;
    upsertToolStep(steps, {
      id: stepId,
      type: "approval",
      title: String(event.payload.tool_name ?? "approval"),
      status: "waiting",
      description,
      time: event.timestamp,
      approvalId,
      toolCallId: toolCallIdFromPayload(event),
      meta: event.payload
    });
    return;
  }

  const approvalId = String(event.payload.approval_id ?? "");
  const stepId = resolveStepId(event);
  upsertToolStep(steps, {
    id: stepId,
    type: "approval",
    title: String(event.payload.tool_name ?? "approval"),
    status: "success",
    time: event.timestamp,
    approvalId,
    toolCallId: toolCallIdFromPayload(event),
    meta: event.payload
  });
}

function ensureAssistantToolMessage(
  messages: ChatMessage[],
  turnId: string | undefined,
  timestamp: string
): ChatMessage {
  const idx = messages.findIndex((message) => message.role === "assistant" && message.turnId === turnId);
  if (idx >= 0) {
    if (!messages[idx].toolSteps) {
      messages[idx] = {
        ...messages[idx],
        toolSteps: []
      };
    }
    return messages[idx];
  }

  const created: ChatMessage = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: "",
    timestamp,
    turnId,
    isDraft: false,
    toolSteps: []
  };
  messages.push(created);
  return created;
}

function resolveStepId(event: StreamEvent, fallbackPrefix = "tool"): string {
  if (event.type === "approval_requested" || event.type === "approval_resolved") {
    const approvalId = String(event.payload.approval_id ?? "");
    if (approvalId.length > 0) {
      return approvalId;
    }
    return `${fallbackPrefix}:approval:${event.turn_id ?? "unknown"}:${event.event_seq}`;
  }
  const toolCallId = toolCallIdFromPayload(event);
  if (toolCallId.length > 0) {
    return toolCallId;
  }
  return `${fallbackPrefix}:${event.turn_id ?? "unknown"}:${event.event_seq}`;
}

function toolCallIdFromPayload(event: StreamEvent): string {
  return String(event.payload.tool_call_id ?? "");
}

function upsertToolStep(steps: ToolStep[], patch: ToolStep): void {
  const idx = steps.findIndex((step) => step.id === patch.id);
  if (idx >= 0) {
    steps[idx] = {
      ...steps[idx],
      ...patch
    };
    return;
  }
  steps.push(patch);
}

function markLatestRunningStepFailed(
  messages: ChatMessage[],
  event: StreamEvent,
  errorMessage: string
): void {
  const turnId = event.turn_id;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.turnId !== turnId || !message.toolSteps || message.toolSteps.length === 0) {
      continue;
    }
    const runningIdx = findLatestRunningToolStepIndex(message.toolSteps);
    if (runningIdx >= 0) {
      message.toolSteps[runningIdx] = {
        ...message.toolSteps[runningIdx],
        status: "failed",
        output: errorMessage,
        time: event.timestamp
      };
      return;
    }
  }

  const toolMessage = ensureAssistantToolMessage(messages, turnId, event.timestamp);
  if (!toolMessage.toolSteps) {
    toolMessage.toolSteps = [];
  }
  upsertToolStep(toolMessage.toolSteps, {
    id: `error:${turnId ?? "unknown"}:${event.event_seq}`,
    type: "summary",
    title: "error",
    status: "failed",
    output: errorMessage,
    time: event.timestamp,
    meta: event.payload
  });
}

function findLatestRunningToolStepIndex(steps: ToolStep[]): number {
  for (let i = steps.length - 1; i >= 0; i -= 1) {
    if (steps[i].type === "tool" && steps[i].status === "running") {
      return i;
    }
  }
  return -1;
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
