import type { ReactNode } from "react";
import { useId, useState } from "react";
import type { ToolStep } from "../../types/chat";
import { formatStepDuration } from "./formatStepDuration";

interface ToolStepCardProps {
  defaultExpanded?: boolean;
  step: ToolStep;
}

export default function ToolStepCard({ defaultExpanded = false, step }: ToolStepCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const reactId = useId().replace(/:/g, "");
  const detailId = `step-detail-${reactId}`;
  const hasBody = Boolean(step.description || step.code || step.output);
  const timeText = formatStepDuration(step);

  const dotClass =
    step.status === "success"
      ? "step-dot step-dot-ok"
      : step.status === "running"
        ? "step-dot step-dot-running"
        : step.status === "failed"
          ? "step-dot step-dot-fail"
          : "step-dot step-dot-wait";

  return (
    <>
      <div
        className={`step-row${hasBody ? " step-row--expandable" : ""}${expanded ? " step-row--open" : ""}`}
        onClick={() => hasBody && setExpanded((prev) => !prev)}
        role={hasBody ? "button" : undefined}
        tabIndex={hasBody ? 0 : undefined}
        onKeyDown={
          hasBody
            ? (e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setExpanded((prev) => !prev);
                }
              }
            : undefined
        }
        aria-expanded={hasBody ? expanded : undefined}
        aria-controls={hasBody ? detailId : undefined}
      >
        <span className={dotClass} aria-hidden="true" />
        <span className="step-name">{step.title || step.type || "tool"}</span>
        {step.subtitle ? (
          <>
            <span className="step-sep" aria-hidden="true">/</span>
            <span className="step-arg">{step.subtitle}</span>
          </>
        ) : null}
        <span className="step-time">{timeText}</span>
        {hasBody ? (
          <svg
            className="step-expand-icon"
            width="11"
            height="11"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <polyline points="18 15 12 9 6 15" />
          </svg>
        ) : null}
      </div>
      {hasBody ? (
        <div
          id={detailId}
          className={`step-detail${expanded ? " open" : ""}`}
          role="region"
        >
          {step.description ? <p className="step-desc">{step.description}</p> : null}
          {step.code ? <pre className="step-code">{renderDiff(step.code)}</pre> : null}
          {step.output ? <pre className="step-output">{step.output}</pre> : null}
        </div>
      ) : null}
    </>
  );
}

/** Render code lines with diff colouring when +/- prefixes are present */
function renderDiff(code: string): ReactNode {
  const lines = code.split("\n");
  const hasDiff = lines.some((l) => l.startsWith("+") || l.startsWith("-"));
  if (!hasDiff) {
    return code;
  }
  return lines.map((line, i) => {
    const cls = line.startsWith("+")
      ? "diff-add"
      : line.startsWith("-")
        ? "diff-del"
        : "diff-ctx";
    return (
      <span key={i} className={cls}>
        {line}
      </span>
    );
  });
}
