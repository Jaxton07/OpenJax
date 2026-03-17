import { useState } from "react";
import type { PendingApproval, ToolStep } from "../../types/chat";
import { formatStepDuration } from "./formatStepDuration";

interface ApprovalStepCardProps {
  step: ToolStep;
  pendingApproval?: PendingApproval;
  onResolve: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

export default function ApprovalStepCard({ step, pendingApproval, onResolve }: ApprovalStepCardProps) {
  const [submitting, setSubmitting] = useState(false);
  const durationText = formatStepDuration(step);

  const submitDecision = async (approved: boolean) => {
    if (!pendingApproval || submitting) {
      return;
    }
    setSubmitting(true);
    try {
      await onResolve(pendingApproval, approved);
    } finally {
      setSubmitting(false);
    }
  };

  // If there's a pending approval, render as a full amber card
  if (pendingApproval) {
    const toolName = step.title || pendingApproval.toolName || "approval";
    const target = pendingApproval.target || step.subtitle || "";
    const reason = pendingApproval.reason || step.description || "";

    return (
      <div className="approval-card" data-testid="approval-step-card">
        <div className="approval-card-hdr">
          {/* Shield icon */}
          <svg
            className="approval-card-icon"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
          </svg>
          <div>
            <div className="approval-card-title">需要确认 · {toolName}</div>
            {(target || reason) ? (
              <div className="approval-card-sub">{reason || target}</div>
            ) : null}
          </div>
        </div>
        {(step.code || step.output) ? (
          <div className="approval-card-code">
            {step.code ? step.code : null}
            {step.output ? step.output : null}
          </div>
        ) : null}
        <div className="approval-card-actions">
          <button
            type="button"
            className="btn-ok"
            onClick={() => void submitDecision(true)}
            disabled={submitting}
          >
            确认
          </button>
          <button
            type="button"
            className="btn-no"
            onClick={() => void submitDecision(false)}
            disabled={submitting}
          >
            取消
          </button>
        </div>
      </div>
    );
  }

  // Resolved approval — render as a compact step row with a "已批准" badge
  const dotClass =
    step.status === "success"
      ? "step-dot step-dot-ok"
      : step.status === "failed"
        ? "step-dot step-dot-fail"
        : "step-dot step-dot-wait";

  const badgeClass =
    step.status === "success"
      ? "step-badge step-badge-approved"
      : step.status === "running"
        ? "step-badge step-badge-running"
        : undefined;

  const badgeText =
    step.status === "success" ? "已批准" : step.status === "running" ? "处理中" : undefined;

  return (
    <div className="step-row" data-testid="approval-step-card">
      <span className={dotClass} aria-hidden="true" />
      <span className="step-name">{step.title || "approval"}</span>
      {step.subtitle ? (
        <>
          <span className="step-sep" aria-hidden="true">/</span>
          <span className="step-arg">{step.subtitle}</span>
        </>
      ) : null}
      {badgeClass && badgeText ? (
        <span className={badgeClass}>{badgeText}</span>
      ) : (
        <span className="step-time">{durationText}</span>
      )}
    </div>
  );
}
