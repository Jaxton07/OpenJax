import { useId, useState } from "react";
import type { ToolStep } from "../../types/chat";
import { formatStepDuration } from "./formatStepDuration";
import StepBody from "./StepBody";
import { RightArrowIcon } from "../../pic/icon";

interface ToolStepCardProps {
  defaultExpanded?: boolean;
  step: ToolStep;
}

export default function ToolStepCard({ defaultExpanded = false, step }: ToolStepCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const reactId = useId().replace(/:/g, "");
  const headingId = `step-heading-${reactId}`;
  const bodyId = `step-body-${reactId}`;
  const hasBody = Boolean(step.description || step.code || step.output);
  const timeText = formatStepDuration(step);

  return (
    <section className={`step-card step-card--${step.status}${expanded ? " expanded" : ""}`}>
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
          <strong className="step-title">{step.title || "tool"}</strong>
          <span className="step-subtitle">{step.subtitle || step.type}</span>
          <span className="step-time">{timeText}</span>
        </span>
        {hasBody ? (
          <RightArrowIcon className="step-chevron" aria-hidden="true" width={14} height={14} />
        ) : null}
      </button>
      <StepBody bodyId={bodyId} expanded={expanded} headingId={headingId} step={step} />
    </section>
  );
}
