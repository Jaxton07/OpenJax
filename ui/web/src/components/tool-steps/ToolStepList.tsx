import type { ToolStep } from "../../types/chat";
import ToolStepCard from "./ToolStepCard";

interface ToolStepListProps {
  steps: ToolStep[];
}

export default function ToolStepList({ steps }: ToolStepListProps) {
  return (
    <div className="tool-step-list" data-testid="tool-step-list">
      {steps.map((step) => (
        <ToolStepCard key={step.id} step={step} />
      ))}
    </div>
  );
}
