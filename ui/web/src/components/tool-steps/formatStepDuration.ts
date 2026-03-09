import type { ToolStep } from "../../types/chat";

export function formatStepDuration(step: ToolStep): string {
  const durationSec = resolveDurationSec(step);
  if (durationSec === undefined) {
    if (step.status === "running" || step.status === "waiting") {
      return "进行中";
    }
    return "-";
  }
  if (durationSec > 60) {
    const mins = Math.floor(durationSec / 60);
    const secs = durationSec % 60;
    return `${mins}m ${secs}s`;
  }
  return `${durationSec}s`;
}

function resolveDurationSec(step: ToolStep): number | undefined {
  if (typeof step.durationSec === "number" && Number.isFinite(step.durationSec) && step.durationSec >= 0) {
    return Math.floor(step.durationSec);
  }
  if (!step.startedAt || !step.endedAt) {
    return undefined;
  }
  const startedMs = Date.parse(step.startedAt);
  const endedMs = Date.parse(step.endedAt);
  if (Number.isNaN(startedMs) || Number.isNaN(endedMs) || endedMs < startedMs) {
    return undefined;
  }
  return Math.floor((endedMs - startedMs) / 1000);
}
