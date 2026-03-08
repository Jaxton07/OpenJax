import { useState } from "react";

interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
  onClear: () => void;
  onCompact: () => void;
}

export default function Composer({ disabled, onSend, onClear, onCompact }: ComposerProps) {
  const [input, setInput] = useState("");

  const submit = async () => {
    const content = input.trim();
    if (!content) {
      return;
    }
    await onSend(content);
    setInput("");
  };

  return (
    <div className="composer-wrap">
      <div className="composer-actions">
        <button onClick={onClear}>清空</button>
        <button onClick={onCompact}>压缩</button>
      </div>
      <div className="composer">
        <textarea
          value={input}
          onChange={(event) => setInput(event.target.value)}
          placeholder="有问题，尽管问"
          rows={1}
          disabled={disabled}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
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
