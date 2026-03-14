import { useState } from "react";

interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
}

export default function Composer({ disabled, onSend }: ComposerProps) {
  const [value, setValue] = useState("");
  const canSend = value.trim().length > 0 && !disabled;

  const submit = async () => {
    const text = value.trim();
    if (!text) {
      return;
    }
    await onSend(text);
    setValue("");
  };

  return (
    <section className="panel composer-panel">
      <textarea
        value={value}
        onChange={(event) => setValue(event.target.value)}
        placeholder="输入消息后回车发送"
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) {
            return;
          }
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            void submit();
          }
        }}
        disabled={disabled}
      />
      <button className="primary" onClick={() => void submit()} disabled={!canSend}>
        Send
      </button>
    </section>
  );
}
