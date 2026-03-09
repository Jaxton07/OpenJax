import { useId, useState } from "react";
import type { PendingApproval, ToolStep } from "../../types/chat";
import { formatStepDuration } from "./formatStepDuration";
import StepBody from "./StepBody";

interface ApprovalStepCardProps {
  step: ToolStep;
  pendingApproval?: PendingApproval;
  onResolve: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

export default function ApprovalStepCard({ step, pendingApproval, onResolve }: ApprovalStepCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const reactId = useId().replace(/:/g, "");
  const headingId = `approval-step-heading-${reactId}`;
  const bodyId = `approval-step-body-${reactId}`;
  const hasBody = Boolean(step.description || step.code || step.output);
  const durationText = formatStepDuration(step);
  const subtitle = pendingApproval?.target || step.subtitle || step.type;

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

  return (
    <section
      className={`step-card approval-step-card step-card--${step.status}${expanded ? " expanded" : ""}`}
      data-testid="approval-step-card"
    >
      <button
        id={headingId}
        type="button"
        className="step-head"
        aria-expanded={expanded}
        aria-controls={hasBody ? bodyId : undefined}
        onClick={() => {
          if (!hasBody) {
            return;
          }
          setExpanded((prev) => !prev);
        }}
      >
        <span className={`step-dot step-dot--${step.status}`} aria-hidden="true" />
        <span className="step-meta">
          <strong className="step-title">{step.title || "approval"}</strong>
          <span className="step-subtitle">{subtitle}</span>
          <span className="step-time">{durationText}</span>
        </span>
      </button>
      <StepBody bodyId={bodyId} expanded={expanded} headingId={headingId} step={step} />
      {pendingApproval ? (
        <div className="approval-step-actions">
          <button type="button" onClick={() => void submitDecision(false)} disabled={submitting}>
            拒绝
          </button>
          <button
            type="button"
            className="primary"
            onClick={() => void submitDecision(true)}
            disabled={submitting}
          >
            允许
          </button>
        </div>
      ) : null}
    </section>
  );
}
