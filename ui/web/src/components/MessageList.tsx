import { useCallback, useEffect, useMemo, useRef } from "react";
import MarkdownRender from "markstream-react";
import { useStreamRenderSnapshot } from "../hooks/useStreamRenderSnapshot";
import { recordMessageListRender } from "../lib/streamPerf";
import type { ChatMessage, PendingApproval } from "../types/chat";
import ToolStepList from "./tool-steps/ToolStepList";

interface MessageListProps {
  sessionId?: string;
  messages: ChatMessage[];
  pendingApprovals: PendingApproval[];
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

type AssistantRenderMode = "text" | "markdown";

export default function MessageList({ sessionId, messages, pendingApprovals, onResolveApproval }: MessageListProps) {
  recordMessageListRender(sessionId);
  const assistantRenderMode = resolveAssistantRenderMode();
  const endRef = useRef<HTMLDivElement | null>(null);
  const shouldStickToBottomRef = useRef(true);
  const lastStreamScrollAtRef = useRef(0);

  const scrollToBottom = useCallback((throttled: boolean) => {
    const anchor = endRef.current;
    if (!anchor) {
      return;
    }
    const container = anchor.closest(".chat-scroll-region");
    if (!(container instanceof HTMLElement)) {
      return;
    }
    if (throttled) {
      const now = Date.now();
      if (now - lastStreamScrollAtRef.current < 120) {
        return;
      }
      lastStreamScrollAtRef.current = now;
    }
    const threshold = 64;
    const distanceToBottom = container.scrollHeight - (container.scrollTop + container.clientHeight);
    shouldStickToBottomRef.current = distanceToBottom <= threshold;
    if (shouldStickToBottomRef.current) {
      container.scrollTop = container.scrollHeight;
    }
  }, []);

  const messageSignature = useMemo(() => {
    const tail = messages.at(-1);
    return `${messages.length}:${tail?.id ?? ""}:${tail?.isDraft ? 1 : 0}:${tail?.content.length ?? 0}`;
  }, [messages]);

  useEffect(() => {
    scrollToBottom(false);
  }, [messageSignature, scrollToBottom]);

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
        <MessageRow
          key={message.id}
          sessionId={sessionId}
          message={message}
          pendingApprovals={pendingApprovals}
          assistantRenderMode={assistantRenderMode}
          onResolveApproval={onResolveApproval}
          onDraftStreamTick={() => scrollToBottom(true)}
          onDraftStreamEnd={() => scrollToBottom(false)}
        />
      ))}
      <div ref={endRef} />
    </div>
  );
}

interface MessageRowProps {
  sessionId?: string;
  message: ChatMessage;
  pendingApprovals: PendingApproval[];
  assistantRenderMode: AssistantRenderMode;
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
  onDraftStreamTick: () => void;
  onDraftStreamEnd: () => void;
}

function MessageRow({
  sessionId,
  message,
  pendingApprovals,
  assistantRenderMode,
  onResolveApproval,
  onDraftStreamTick,
  onDraftStreamEnd
}: MessageRowProps) {
  return (
    <div className={`message-row role-${message.role}`}>
      <div className="message-bubble">
        {message.kind === "tool_steps" ? (
          Array.isArray(message.toolSteps) && message.toolSteps.length > 0 ? (
            <ToolStepList
              steps={message.toolSteps}
              pendingApprovals={pendingApprovals}
              onResolveApproval={onResolveApproval}
            />
          ) : null
        ) : message.role === "assistant" && message.isDraft && message.turnId ? (
          <AssistantDraftMessage
            sessionId={sessionId}
            turnId={message.turnId}
            fallbackContent={message.content}
            assistantRenderMode={assistantRenderMode}
            onStreamTick={onDraftStreamTick}
            onStreamEnd={onDraftStreamEnd}
          />
        ) : message.role === "assistant" ? (
          <AssistantMessage content={message.content} mode={assistantRenderMode} final />
        ) : (
          <div className="message-text">{message.content}</div>
        )}
      </div>
    </div>
  );
}

interface AssistantDraftMessageProps {
  sessionId?: string;
  turnId: string;
  fallbackContent: string;
  assistantRenderMode: AssistantRenderMode;
  onStreamTick: () => void;
  onStreamEnd: () => void;
}

function AssistantDraftMessage({
  sessionId,
  turnId,
  fallbackContent,
  assistantRenderMode,
  onStreamTick,
  onStreamEnd
}: AssistantDraftMessageProps) {
  const snapshot = useStreamRenderSnapshot(sessionId, turnId);
  const content = snapshot.version > 0 ? snapshot.content : fallbackContent;

  useEffect(() => {
    if (snapshot.version === 0) {
      return;
    }
    if (snapshot.isActive) {
      onStreamTick();
      return;
    }
    onStreamEnd();
  }, [onStreamEnd, onStreamTick, snapshot.isActive, snapshot.version]);

  return <AssistantMessage content={content} mode={assistantRenderMode} final={!snapshot.isActive} />;
}

function AssistantMessage({ content, mode, final }: { content: string; mode: AssistantRenderMode; final: boolean }) {
  if (mode === "markdown") {
    return (
      <div className="assistant-markdown">
        <MarkdownRender
          content={content}
          final={final}
          batchRendering={false}
          deferNodesUntilVisible={false}
        />
      </div>
    );
  }
  return <div className="message-text">{content}</div>;
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
