import type { ToolStep } from "../../types/chat";

interface StepBodyProps {
  bodyId: string;
  expanded: boolean;
  headingId: string;
  step: ToolStep;
}

export default function StepBody({ bodyId, expanded, headingId, step }: StepBodyProps) {
  const hasDescription = Boolean(step.description);
  const hasCode = Boolean(step.code);
  const hasOutput = Boolean(step.output);
  if (!hasDescription && !hasCode && !hasOutput) {
    return null;
  }

  return (
    <div
      id={bodyId}
      className={`step-body${expanded ? " expanded" : ""}`}
      role="region"
      aria-labelledby={headingId}
    >
      {hasDescription ? <p className="step-desc">{step.description}</p> : null}
      {hasCode ? <pre className="step-code">{step.code}</pre> : null}
      {hasOutput ? <pre className="step-output">{step.output}</pre> : null}
    </div>
  );
}
