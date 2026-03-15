import { useLayoutEffect, useRef, useState } from "react";
import { CompactIcon, PlusIcon, SendIcon } from "../pic/icon";

interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onCompact: () => void;
}

export default function Composer({ disabled, onSend, onNewChat, onCompact }: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const hasContent = input.trim().length > 0;

  const resizeTextarea = () => {
    const el = textareaRef.current;
    if (!el) {
      return;
    }
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
  };

  useLayoutEffect(() => {
    resizeTextarea();
  }, [input]);

  const submit = async () => {
    const content = input.trim();
    if (!content) {
      return;
    }
    await onSend(content);
    setInput("");
    const el = textareaRef.current;
    if (el) {
      el.style.height = "";
    }
  };

  return (
    <div className="composer-wrap">
      <div className="composer-actions">
        <button onClick={onNewChat}>
          <span className="composer-action-icon" aria-hidden="true">
            <PlusIcon />
          </span>
          <span>新建对话</span>
        </button>
        <button onClick={onCompact}>
          <span className="composer-action-icon" aria-hidden="true">
            <CompactIcon />
          </span>
          <span>压缩</span>
        </button>
      </div>
      <div className="composer">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(event) => setInput(event.target.value)}
          placeholder="有问题，尽管问"
          rows={1}
          disabled={disabled}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) {
              return;
            }
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void submit();
            }
          }}
        />
        <button
          type="button"
          className={`composer-send-btn ${hasContent && !disabled ? "ready" : ""}`}
          onClick={() => void submit()}
          disabled={disabled || !hasContent}
          aria-label="发送"
          title="发送"
        >
          <SendIcon aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
