import { useEffect, useRef } from "react";
import MarkdownRender from "markstream-react";
import type { ChatMessage, PendingApproval, SessionStreamingState } from "../types/chat";
import ToolStepList from "./tool-steps/ToolStepList";

interface MessageListProps {
  messages: ChatMessage[];
  pendingApprovals: PendingApproval[];
  streaming?: SessionStreamingState;
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

type AssistantRenderMode = "text" | "markdown";

export default function MessageList({
  messages,
  pendingApprovals,
  streaming,
  onResolveApproval
}: MessageListProps) {
  const assistantRenderMode = resolveAssistantRenderMode();
  const endRef = useRef<HTMLDivElement | null>(null);
  const shouldStickToBottomRef = useRef(true);

  useEffect(() => {
    const anchor = endRef.current;
    if (!anchor) {
      return;
    }
    const container = anchor.closest(".chat-scroll-region");
    if (!(container instanceof HTMLElement)) {
      return;
    }
    const threshold = 64;
    const distanceToBottom = container.scrollHeight - (container.scrollTop + container.clientHeight);
    shouldStickToBottomRef.current = distanceToBottom <= threshold;
    if (shouldStickToBottomRef.current) {
      container.scrollTop = container.scrollHeight;
    }
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
      {messages.map((message) => {
        const resolvedContent =
          message.role === "assistant" &&
          message.kind === "text" &&
          message.isDraft &&
          message.turnId &&
          streaming?.active &&
          streaming.turnId === message.turnId
            ? streaming.content
            : message.content;
        return (
          <div key={message.id} className={`message-row role-${message.role}`}>
            <div className="message-bubble">
              {message.kind === "tool_steps" ? (
                Array.isArray(message.toolSteps) && message.toolSteps.length > 0 ? (
                  <ToolStepList
                    steps={message.toolSteps}
                    pendingApprovals={pendingApprovals}
                    onResolveApproval={onResolveApproval}
                  />
                ) : null
              ) : message.role === "assistant" ? (
                assistantRenderMode === "markdown" ? (
                  <div className="assistant-markdown">
                    <MarkdownRender
                      content={resolvedContent}
                      final={!message.isDraft}
                      batchRendering={false}
                      deferNodesUntilVisible={false}
                    />
                  </div>
                ) : (
                  <div className="message-text">{resolvedContent}</div>
                )
              ) : (
                <div className="message-text">{resolvedContent}</div>
              )}
            </div>
          </div>
        );
      })}
      <div ref={endRef} />
    </div>
  );
}

function resolveAssistantRenderMode(): AssistantRenderMode {
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string;
          VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string;
        })
      : {};
  const raw = String(
    globals.OPENJAX_WEB_ASSISTANT_RENDER_MODE ??
      globals.VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE ??
      "text"
  )
    .trim()
    .toLowerCase();
  return raw === "markdown" ? "markdown" : "text";
}
