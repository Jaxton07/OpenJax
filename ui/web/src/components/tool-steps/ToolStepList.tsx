import type { PendingApproval, ToolStep } from "../../types/chat";
import ApprovalStepCard from "./ApprovalStepCard";
import ToolStepCard from "./ToolStepCard";

interface ToolStepListProps {
  steps: ToolStep[];
  pendingApprovals: PendingApproval[];
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

export default function ToolStepList({ steps, pendingApprovals, onResolveApproval }: ToolStepListProps) {
  // Separate pending approval steps (rendered as full amber cards outside the group)
  // from regular steps (rendered as compact rows inside a bordered group)
  const pendingApprovalSteps: Array<{ step: ToolStep; approval: PendingApproval }> = [];
  const regularSteps: ToolStep[] = [];

  for (const step of steps) {
    const pendingApproval = resolvePendingApproval(step, pendingApprovals);
    if (pendingApproval) {
      pendingApprovalSteps.push({ step, approval: pendingApproval });
    } else {
      regularSteps.push(step);
    }
  }

  return (
    <div className="tool-step-list" data-testid="tool-step-list">
      {/* Regular steps grouped in a bordered container */}
      {regularSteps.length > 0 ? (
        <div className="tool-group">
          {regularSteps.map((step) =>
            step.type === "approval" ? (
              <ApprovalStepCard
                key={step.id}
                step={step}
                pendingApproval={undefined}
                onResolve={onResolveApproval}
              />
            ) : (
              <ToolStepCard key={step.id} step={step} />
            )
          )}
        </div>
      ) : null}

      {/* Pending approvals rendered as full amber cards below the group */}
      {pendingApprovalSteps.map(({ step, approval }) => (
        <ApprovalStepCard
          key={step.id}
          step={step}
          pendingApproval={approval}
          onResolve={onResolveApproval}
        />
      ))}
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
