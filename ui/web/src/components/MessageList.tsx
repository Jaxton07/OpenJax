import { useEffect, useRef } from "react";
import type { ChatMessage } from "../types/chat";

interface MessageListProps {
  messages: ChatMessage[];
}

export default function MessageList({ messages }: MessageListProps) {
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div className="welcome-panel">
        <h1>你好，准备好开始了吗？</h1>
        <p>从左侧新建会话，或在下方输入框直接提问。</p>
      </div>
    );
  }

  return (
    <div className="message-list">
      {messages.map((message) => (
        <div key={message.id} className={`message-row role-${message.role}`}>
          <div className="message-bubble">
            <pre>{message.content}</pre>
          </div>
        </div>
      ))}
      <div ref={endRef} />
    </div>
  );
}
