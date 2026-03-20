import type { RefObject } from "react";
import { SendIcon } from "../../pic/icon";

interface ComposerInputProps {
  value: string;
  disabled?: boolean;
  textareaRef: RefObject<HTMLTextAreaElement>;
  onChange: (value: string) => void;
  onSubmit: () => void;
}

export default function ComposerInput({
  value,
  disabled,
  textareaRef,
  onChange,
  onSubmit,
}: ComposerInputProps) {
  const hasContent = value.trim().length > 0;

  return (
    <div className="composer">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="有问题，尽管问"
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
