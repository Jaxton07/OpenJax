import type { RefObject } from "react";
import { SendIcon } from "../../pic/icon";
import type { ContextUsageState } from "../../types/chat";
import type { SlashCommandDto } from "../../types/gateway";
import ContextUsageRing from "./ContextUsageRing";

interface ComposerInputProps {
  value: string;
  disabled?: boolean;
  textareaRef: RefObject<HTMLTextAreaElement>;
  onChange: (value: string) => void;
  onSubmit: () => void;
  contextUsage?: ContextUsageState | null;
  showSlashDropdown?: boolean;
  slashCommands?: SlashCommandDto[];
  slashSelectedIndex?: number;
  onSlashIndexChange?: (index: number) => void;
  onSlashClose?: () => void;
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
}: ComposerInputProps) {
  const hasContent = value.trim().length > 0;

  return (
    <div className="composer">
      <ContextUsageRing contextUsage={contextUsage} />
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
    </div>
  );
}
