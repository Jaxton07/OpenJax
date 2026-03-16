import type { ChatMessage, ToolStep } from "../../types/chat";
import type { StreamEvent } from "../../types/gateway";

export function isToolStepEvent(event: StreamEvent): boolean {
  return (
    event.type === "tool_call_started" ||
    event.type === "tool_call_completed" ||
    event.type === "approval_requested" ||
    event.type === "approval_resolved" ||
    event.type === "error"
  );
}

export function upsertToolStepMessage(messages: ChatMessage[], event: StreamEvent): void {
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

export function toolCallIdFromPayload(event: StreamEvent): string {
  return String(event.payload.tool_call_id ?? "");
}

export function batchStatusFromPayload(payload: Record<string, unknown>): ToolStep["status"] {
  const failed = Number(payload.failed ?? 0);
  return failed > 0 ? "failed" : "success";
}

export function upsertToolBatchSummary(
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

type AggregateKey = { type: "tool_call_id" | "approval_id"; value: string } | null;

function findToolStepMessageIndex(messages: ChatMessage[], event: StreamEvent): number {
  const key = resolveAggregationKey(event);
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
