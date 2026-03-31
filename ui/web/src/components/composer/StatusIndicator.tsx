import "../../styles/animations/thinking.css";
import "../../styles/animations/working.css";
import "./status-indicator.css";
import type { ComposerPhase } from "../../lib/deriveComposerPhase";

interface StatusIndicatorProps {
  phase: ComposerPhase;
}

export default function StatusIndicator({ phase }: StatusIndicatorProps) {
  const visible = phase !== "idle";

  return (
    <div className={`status-indicator${visible ? " visible" : ""}`} aria-live="polite" aria-atomic="true">
      {phase === "thinking" && (
        <>
          <div className="cyber-liquid-4">
            <div className="inner" />
          </div>
        </>
      )}
      {phase === "working" && (
        <>
          <span className="status-indicator-label">Working</span>
          <div className="scan-bar" />
        </>
      )}
    </div>
  );
}
