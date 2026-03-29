import type { RefObject } from "react";
import { SendIcon, StopCircleIcon } from "../../pic/icon";
import type { ContextUsageState } from "../../types/chat";
import ContextUsageRing from "./ContextUsageRing";
import PolicyLevelButton from "./PolicyLevelButton";

interface ComposerInputProps {
  value: string;
  disabled?: boolean;
  textareaRef: RefObject<HTMLTextAreaElement>;
  onChange: (value: string) => void;
  onSubmit: () => void;
  contextUsage?: ContextUsageState | null;
  showSlashDropdown?: boolean;
  slashSelectedIndex?: number;
  onSlashIndexChange?: (index: number) => void;
  onSlashClose?: () => void;
  onSlashSubmit?: () => void;
  policyLevel?: "allow" | "ask" | "deny";
  onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
  isStreaming?: boolean;
  onStop?: () => void;
}

export default function ComposerInput({
  value,
  disabled,
  textareaRef,
  onChange,
  onSubmit,
  contextUsage,
  showSlashDropdown,
  slashSelectedIndex,
  onSlashIndexChange,
  onSlashClose,
  onSlashSubmit,
  policyLevel,
  onPolicyLevelChange,
  isStreaming,
  onStop
}: ComposerInputProps) {
  const hasContent = value.trim().length > 0;

  return (
    <div className="composer">
      <ContextUsageRing contextUsage={contextUsage} />
      {onPolicyLevelChange && (
        <PolicyLevelButton level={policyLevel ?? "ask"} onChange={onPolicyLevelChange} />
      )}
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Ask anything (type / for commands)"
        rows={1}
        disabled={disabled}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) {
            return;
          }
          if (event.key === "Enter" && !event.shiftKey && !disabled) {
            event.preventDefault();
            if (showSlashDropdown && onSlashSubmit) {
              onSlashSubmit();
              return;
            }
            onSubmit();
          }
          if (showSlashDropdown && onSlashIndexChange && onSlashClose) {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              onSlashIndexChange((slashSelectedIndex ?? 0) + 1);
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              onSlashIndexChange((slashSelectedIndex ?? 0) - 1);
            }
            if (event.key === "Escape") {
              onSlashClose();
            }
          }
        }}
      />
      {isStreaming ? (
        <button
          type="button"
          className="composer-stop-btn"
          onClick={onStop}
          aria-label="停止"
          title="停止"
        >
          <StopCircleIcon aria-hidden="true" />
        </button>
      ) : (
        <button
          type="button"
          className={`composer-send-btn ${hasContent && !disabled ? "ready" : ""}`}
          onClick={onSubmit}
          disabled={disabled || !hasContent}
          aria-label="发送"
          title="发送"
        >
          <SendIcon aria-hidden="true" />
        </button>
      )}
    </div>
  );
}
