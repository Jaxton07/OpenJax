import type { PendingApproval, ToolStep } from "../../types/chat";
import ApprovalStepCard from "./ApprovalStepCard";
import ToolStepCard from "./ToolStepCard";

interface ToolStepListProps {
  steps: ToolStep[];
  pendingApprovals: PendingApproval[];
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

export default function ToolStepList({ steps, pendingApprovals, onResolveApproval }: ToolStepListProps) {
  return (
    <div className="tool-step-list" data-testid="tool-step-list">
      {steps.map((step) => {
        const pendingApproval = resolvePendingApproval(step, pendingApprovals);
        const renderApprovalCard = Boolean(pendingApproval) || step.type === "approval";
        if (renderApprovalCard) {
          return (
            <ApprovalStepCard
              key={step.id}
              step={step}
              pendingApproval={pendingApproval}
              onResolve={onResolveApproval}
            />
          );
        }
        return <ToolStepCard key={step.id} step={step} />;
      })}
    </div>
  );
}

function resolvePendingApproval(step: ToolStep, pendingApprovals: PendingApproval[]): PendingApproval | undefined {
  if (step.approvalId) {
    const byApprovalId = pendingApprovals.find((item) => item.approvalId === step.approvalId);
    if (byApprovalId) {
      return byApprovalId;
    }
  }
  if (step.toolCallId) {
    return pendingApprovals.find((item) => item.toolCallId === step.toolCallId);
  }
  return undefined;
}
