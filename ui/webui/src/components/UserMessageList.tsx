import { memo } from "react";
import type { UserMessage } from "../types/chat";

interface UserMessageListProps {
  messages: UserMessage[];
}

function UserMessageList({ messages }: UserMessageListProps) {
  return (
    <section className="panel user-panel">
      <h3>User Messages</h3>
      {messages.length === 0 ? <div className="empty">还没有用户消息</div> : null}
      <div className="user-message-list">
        {messages.map((message) => (
          <div key={message.id} className="user-bubble">
            {message.content}
          </div>
        ))}
      </div>
    </section>
  );
}

export default memo(UserMessageList);
