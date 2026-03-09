import type { ToolStepStatus } from "../../types/chat";

interface StepStatusBadgeProps {
  status: ToolStepStatus;
}

export default function StepStatusBadge({ status }: StepStatusBadgeProps) {
  return <span className={`step-status step-status--${status}`}>{status}</span>;
}
