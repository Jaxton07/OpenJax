import { useCallback, useEffect, useMemo, useRef } from "react";
import MarkdownRender from "markstream-react";
import { useStreamRenderSnapshot } from "../hooks/useStreamRenderSnapshot";
import { sanitizeMarkdownContent } from "../lib/markdown";
import { buildTimeline } from "../lib/timeline/buildTimeline";
import { recordMessageListRender } from "../lib/streamPerf";
import type { ChatMessage, PendingApproval } from "../types/chat";
import ToolStepList from "./tool-steps/ToolStepList";
import ReasoningBlockCard from "./ReasoningBlockCard";
import type { TimelineItem } from "../lib/timeline/types";

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
  const timelineItems = useMemo(() => buildTimeline(messages), [messages]);
  const endRef = useRef<HTMLDivElement | null>(null);
  const shouldStickToBottomRef = useRef(true);
  const lastStreamScrollAtRef = useRef(0);
  const isScrollingProgrammaticallyRef = useRef(false);

  const scrollToBottom = useCallback((throttled: boolean) => {
    if (!shouldStickToBottomRef.current) {
      return;
    }
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
    isScrollingProgrammaticallyRef.current = true;
    container.scrollTop = container.scrollHeight;
    requestAnimationFrame(() => {
      isScrollingProgrammaticallyRef.current = false;
    });
  }, []);

  // Track user-initiated scrolls to enable/disable sticky following
  useEffect(() => {
    const anchor = endRef.current;
    if (!anchor) return;
    const container = anchor.closest(".chat-scroll-region");
    if (!(container instanceof HTMLElement)) return;

    const NEAR_BOTTOM_THRESHOLD = 150;

    const onScroll = () => {
      if (isScrollingProgrammaticallyRef.current) return;
      const distanceToBottom = container.scrollHeight - (container.scrollTop + container.clientHeight);
      shouldStickToBottomRef.current = distanceToBottom <= NEAR_BOTTOM_THRESHOLD;
    };

    container.addEventListener("scroll", onScroll, { passive: true });
    return () => container.removeEventListener("scroll", onScroll);
  }, []);

  const messageSignature = useMemo(() => {
    const tail = timelineItems.at(-1);
    return `${timelineItems.length}:${tail?.id ?? ""}:${tail?.eventSeqEnd ?? 0}`;
  }, [timelineItems]);

  useEffect(() => {
    scrollToBottom(false);
  }, [messageSignature, scrollToBottom]);

  if (timelineItems.length === 0) {
    return (
      <div className="welcome-panel">
        <h1>你好，准备好开始了吗？</h1>
        <p>从左侧新建会话，或在下方输入框直接提问。</p>
      </div>
    );
  }

  return (
    <div className="message-list">
      {timelineItems.map((item) => (
        <TimelineRow
          key={item.id}
          sessionId={sessionId}
          item={item}
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

interface TimelineRowProps {
  sessionId?: string;
  item: TimelineItem;
  pendingApprovals: PendingApproval[];
  assistantRenderMode: AssistantRenderMode;
  onResolveApproval: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
  onDraftStreamTick: () => void;
  onDraftStreamEnd: () => void;
}

function TimelineRow({
  sessionId,
  item,
  pendingApprovals,
  assistantRenderMode,
  onResolveApproval,
  onDraftStreamTick,
  onDraftStreamEnd
}: TimelineRowProps) {
  if (item.type === "reasoning_block") {
    return (
      <div className="message-row role-assistant message-row--compact">
        <div className="message-bubble">
          <ReasoningBlockCard block={item.payload.block} />
        </div>
      </div>
    );
  }

  if (item.type === "tool_step") {
    return (
      <div className="message-row role-assistant message-row--compact">
        <div className="message-bubble">
          <ToolStepList
            steps={[item.payload.step]}
            pendingApprovals={pendingApprovals}
            onResolveApproval={onResolveApproval}
          />
        </div>
      </div>
    );
  }

  const message = item.payload.message;
  return (
    <div className={`message-row role-${message.role}`}>
      <div className="message-bubble">
        {message.role === "assistant" && message.isDraft && message.turnId ? (
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

  return (
    <AssistantMessage
      content={content}
      mode={assistantRenderMode}
      final={!snapshot.isActive}
    />
  );
}

function AssistantMessage({
  content,
  mode,
  final
}: {
  content: string;
  mode: AssistantRenderMode;
  final: boolean;
}) {
  if (mode === "markdown") {
    return (
      <>
        <div className="assistant-markdown">
          <MarkdownRender
            content={sanitizeMarkdownContent(content)}
            final={final}
            batchRendering={false}
            deferNodesUntilVisible={false}
          />
        </div>
      </>
    );
  }
  return (
    <>
      <div className="message-text">{content}</div>
    </>
  );
}



function resolveAssistantRenderMode(): AssistantRenderMode {
  const env =
    typeof import.meta !== "undefined"
      ? (import.meta as { env?: Record<string, unknown> }).env ?? {}
      : {};
  const globals =
    typeof globalThis !== "undefined"
      ? (globalThis as {
          OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string;
          VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string;
        })
      : {};
  const raw = String(
    env.VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE ??
    globals.OPENJAX_WEB_ASSISTANT_RENDER_MODE ??
    globals.VITE_OPENJAX_WEB_ASSISTANT_RENDER_MODE ??
    "markdown"
  )
    .trim()
    .toLowerCase();
  return raw === "text" ? "text" : "markdown";
}
