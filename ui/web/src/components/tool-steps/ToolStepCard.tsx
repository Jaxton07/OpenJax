import { useId, useState } from "react";
import type { ToolStep } from "../../types/chat";
import StepBody from "./StepBody";

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
  const timeText = formatStepTime(step.time);

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
      </button>
      <StepBody bodyId={bodyId} expanded={expanded} headingId={headingId} step={step} />
    </section>
  );
}

function formatStepTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  const hh = String(date.getHours()).padStart(2, "0");
  const mm = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  const ms = String(date.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}
