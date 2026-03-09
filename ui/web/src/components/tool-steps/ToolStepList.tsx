import type { ToolStep } from "../../types/chat";

interface ToolStepListProps {
  steps: ToolStep[];
}

export default function ToolStepList({ steps }: ToolStepListProps) {
  return (
    <div className="tool-step-list" data-testid="tool-step-list">
      {steps.map((step) => (
        <section key={step.id} className={`tool-step-item step-card step-card--${step.status}`}>
          <header className="tool-step-head">
            <strong className="tool-step-title">{step.title || "tool"}</strong>
            <span className={`tool-step-status step-status step-status--${step.status}`}>{step.status}</span>
          </header>
          <p className="tool-step-meta">
            {step.type} · {step.time}
          </p>
          {step.subtitle ? <p className="tool-step-subtitle">{step.subtitle}</p> : null}
          {step.description ? <p className="tool-step-description">{step.description}</p> : null}
          {step.code ? <pre className="tool-step-code">{step.code}</pre> : null}
          {step.output ? <pre className="tool-step-output">{step.output}</pre> : null}
        </section>
      ))}
    </div>
  );
}
