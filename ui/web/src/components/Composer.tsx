import { useLayoutEffect, useRef, useState } from "react";

interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onCompact: () => void;
}

export default function Composer({ disabled, onSend, onNewChat, onCompact }: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

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
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
              <path d="M11 3a1 1 0 0 1 2 0v8h8a1 1 0 1 1 0 2h-8v8a1 1 0 1 1-2 0v-8H3a1 1 0 0 1 0-2h8z" />
            </svg>
          </span>
          <span>新建对话</span>
        </button>
        <button onClick={onCompact}>
          <span className="composer-action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
              <path d="M4 6h16v2H4zM4 11h12v2H4zM4 16h9v2H4z" />
            </svg>
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
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void submit();
            }
          }}
        />
        <button onClick={() => void submit()} disabled={disabled}>
          发送
        </button>
      </div>
    </div>
  );
}
