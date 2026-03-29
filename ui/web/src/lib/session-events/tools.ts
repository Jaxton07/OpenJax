import type { ChatMessage, ToolStep, ToolStepMeta } from "../../types/chat";
import type {
  ShellExecutionMetadata,
  StreamEvent,
  ToolCallCompletedPayload,
  ToolCallStartedPayload
} from "../../types/gateway";

export function isToolStepEvent(event: StreamEvent): boolean {
  return (
    event.type === "tool_call_started" ||
    event.type === "tool_call_ready" ||
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
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
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
    startEventSeq: prevMessage.startEventSeq ?? event.event_seq,
    lastEventSeq: Math.max(prevMessage.lastEventSeq ?? event.event_seq, event.event_seq),
    toolSteps: [mergeToolStep(prevStep, nextStep)]
  };
}

export function toolCallIdFromPayload(event: StreamEvent): string {
  return String(event.payload.tool_call_id ?? "");
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
      startEventSeq: previous.startEventSeq,
      lastEventSeq: Math.max(previous.lastEventSeq ?? next.lastEventSeq ?? 0, next.lastEventSeq ?? 0),
      endEventSeq: next.endEventSeq ?? previous.endEventSeq,
      meta: next.meta
    });
  }
  return withDuration({
    ...previous,
    ...next,
    startEventSeq: previous.startEventSeq,
    lastEventSeq: Math.max(previous.lastEventSeq ?? next.lastEventSeq ?? 0, next.lastEventSeq ?? 0),
    endEventSeq: next.endEventSeq ?? previous.endEventSeq
  });
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
      title: toolStartedPayload(event).display_name ?? String(toolStartedPayload(event).tool_name ?? "tool"),
      target: String(toolStartedPayload(event).target ?? ""),
      status: "running",
      time: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      startedAt: event.timestamp,
      toolCallId: toolCallIdFromPayload(event),
      meta: { rawPayload: event.payload }
    };
  }

  if (event.type === "tool_call_ready") {
    const payload = event.payload as { tool_call_id?: string; target?: string };
    const target = payload.target ?? "";
    return {
      id: toolCallIdFromPayload(event) || `tool_call_ready:${event.turn_id ?? "unknown"}:${event.event_seq}`,
      type: "tool" as const,
      target,
      toolCallId: toolCallIdFromPayload(event),
    } as ToolStep;
  }

  if (event.type === "tool_call_completed") {
    const payload = toolCompletedPayload(event);
    const shellMetadata = payload.shell_metadata;
    const partial = isPartialResult(shellMetadata, payload.output);
    const meta = buildToolStepMeta(event.payload, shellMetadata, partial, payload.output);
    return {
      id: resolveToolStepId(event, "tool_call_completed"),
      type: "tool",
      title: payload.display_name ?? String(payload.tool_name ?? "tool"),
      status: payload.ok === false ? "failed" : "success",
      description: partial ? partialDescription(shellMetadata) : undefined,
      output: String(payload.output ?? ""),
      time: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      endEventSeq: event.event_seq,
      endedAt: event.timestamp,
      toolCallId: toolCallIdFromPayload(event),
      meta
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
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      startedAt: event.timestamp,
      approvalId: String(event.payload.approval_id ?? ""),
      toolCallId: toolCallIdFromPayload(event),
      meta: { rawPayload: event.payload }
    };
  }

  if (event.type === "approval_resolved") {
    return {
      id: resolveToolStepId(event, "approval_resolved"),
      type: "approval",
      title: String(event.payload.tool_name ?? "approval"),
      status: "success",
      time: event.timestamp,
      startEventSeq: event.event_seq,
      lastEventSeq: event.event_seq,
      endEventSeq: event.event_seq,
      endedAt: event.timestamp,
      approvalId: String(event.payload.approval_id ?? ""),
      toolCallId: toolCallIdFromPayload(event),
      meta: { rawPayload: event.payload }
    };
  }

  return {
    id: `error:${event.turn_id ?? "unknown"}:${event.event_seq}`,
    type: "summary",
    title: "error",
    status: "failed",
    output: String(event.payload.message ?? "turn failed"),
    time: event.timestamp,
    startEventSeq: event.event_seq,
    lastEventSeq: event.event_seq,
    endEventSeq: event.event_seq,
    meta: { rawPayload: event.payload }
  };
}

export function toolStartedPayload(event: StreamEvent): ToolCallStartedPayload {
  return event.payload as ToolCallStartedPayload;
}

export function toolCompletedPayload(event: StreamEvent): ToolCallCompletedPayload {
  return event.payload as ToolCallCompletedPayload;
}

function buildToolStepMeta(
  rawPayload: Record<string, unknown>,
  shellMetadata: ShellExecutionMetadata | undefined,
  partial: boolean,
  output: string | undefined
): ToolStepMeta {
  return {
    rawPayload,
    shellMetadata,
    backendSummary: extractBackendSummary(shellMetadata, output),
    riskSummary: degradedRiskSummary(shellMetadata, output),
    hint: skillTriggerGuardHint(shellMetadata, output),
    partial
  };
}

function partialDescription(shellMetadata: ShellExecutionMetadata | undefined): string {
  if (!shellMetadata) {
    return "Partial success";
  }
  return `Partial success (exit code ${shellMetadata.exit_code})`;
}

function isPartialResult(
  shellMetadata: ShellExecutionMetadata | undefined,
  output: string | undefined
): boolean {
  return (
    shellMetadata?.result_class === "partial_success" ||
    String(output ?? "").includes("result_class=partial_success")
  );
}

function extractBackendSummary(
  shellMetadata: ShellExecutionMetadata | undefined,
  output: string | undefined
): string | undefined {
  const backend = shellMetadata?.backend ?? findOutputField(output, "backend");
  if (!backend) {
    return undefined;
  }
  return `sandbox: ${backendLabel(backend)}`;
}

function backendLabel(backend: string): string {
  switch (backend) {
    case "macos_seatbelt":
      return "sandbox-exec (macos_seatbelt)";
    case "linux_native":
      return "bwrap (linux_native)";
    case "none_escalated":
      return "none (degraded)";
    default:
      return backend;
  }
}

function degradedRiskSummary(
  shellMetadata: ShellExecutionMetadata | undefined,
  output: string | undefined
): string | undefined {
  const backend = shellMetadata?.backend ?? findOutputField(output, "backend");
  if (backend !== "none_escalated") {
    return undefined;
  }
  const command = (findOutputField(output, "command") ?? "").toLowerCase();
  const policyDecision = (
    shellMetadata?.policy_decision ??
    findOutputField(output, "policy_decision") ??
    ""
  ).toLowerCase();
  const mutating = isMutatingCommand(command) || policyDecision.includes("askapproval");
  return mutating ? "risk: mutating command ran unsandboxed" : "degraded: executed outside sandbox";
}

function skillTriggerGuardHint(
  shellMetadata: ShellExecutionMetadata | undefined,
  output: string | undefined
): string | undefined {
  const denyReason =
    shellMetadata?.runtime_deny_reason ?? findOutputField(output, "runtime_deny_reason");
  return denyReason === "skill_trigger_not_shell_command"
    ? "hint: detected skill trigger string in shell; use skill workflow steps"
    : undefined;
}

function findOutputField(output: string | undefined, field: string): string | undefined {
  return output
    ?.split("\n")
    .find((line) => line.startsWith(`${field}=`))
    ?.slice(field.length + 1)
    .trim();
}

function isMutatingCommand(command: string): boolean {
  return [
    "git add ",
    "git commit",
    "git merge",
    "git rebase",
    "git cherry-pick",
    "git reset --hard",
    "git clean -fd",
    "rm ",
    "mv ",
    "cp ",
    "chmod ",
    "chown ",
    "touch ",
    "mkdir ",
    "rmdir ",
    "sed -i",
    "perl -i",
    "truncate ",
    ">",
    ">>"
  ].some((token) => command.includes(token));
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
