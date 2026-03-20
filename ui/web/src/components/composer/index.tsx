import { useLayoutEffect, useRef, useState } from "react";
import "./composer.css";
import ComposerActions from "./ComposerActions";
import ComposerInput from "./ComposerInput";

interface ComposerProps {
  disabled?: boolean;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onCompact: () => void;
}

export default function Composer({ disabled, onSend, onNewChat, onCompact }: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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
      <ComposerActions onNewChat={onNewChat} onCompact={onCompact} />
      <ComposerInput
        value={input}
        disabled={disabled}
        textareaRef={textareaRef}
        onChange={setInput}
        onSubmit={() => void submit()}
      />
    </div>
  );
}
