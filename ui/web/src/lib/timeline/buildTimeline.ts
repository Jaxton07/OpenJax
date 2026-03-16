import type { ChatMessage } from "../../types/chat";
import type { TimelineItem } from "./types";

const FALLBACK_EVENT_SEQ = Number.MAX_SAFE_INTEGER;

export function buildTimeline(messages: ChatMessage[]): TimelineItem[] {
  const items: TimelineItem[] = [];
  let stableIndex = 0;

  for (const message of messages) {
    if (message.kind === "tool_steps") {
      const steps = message.toolSteps ?? [];
      for (const step of steps) {
        const eventSeqStart = step.startEventSeq ?? message.startEventSeq ?? FALLBACK_EVENT_SEQ;
        const eventSeqEnd = step.endEventSeq ?? step.lastEventSeq ?? eventSeqStart;
        items.push({
          id: `tool_step:${message.id}:${step.id}`,
          type: "tool_step",
          turnId: message.turnId,
          eventSeqStart,
          eventSeqEnd,
          timestamp: step.time || message.timestamp,
          stableIndex: stableIndex++,
          payload: { step, message }
        });
      }
      continue;
    }

    if (message.role === "assistant") {
      const blocks = message.reasoningBlocks ?? [];
      for (let i = 0; i < blocks.length; i += 1) {
        const block = blocks[i];
        const eventSeqStart = block.startEventSeq ?? message.startEventSeq ?? FALLBACK_EVENT_SEQ;
        const eventSeqEnd = block.endEventSeq ?? block.lastEventSeq ?? eventSeqStart;
        items.push({
          id: `reasoning_block:${message.id}:${block.blockId}`,
          type: "reasoning_block",
          turnId: message.turnId,
          eventSeqStart,
          eventSeqEnd,
          timestamp: block.startedAt || message.timestamp,
          stableIndex: stableIndex++,
          payload: {
            block,
            message,
            sequenceNumber: i + 1
          }
        });
      }

      if (message.content || message.isDraft) {
        const eventSeqStart = message.startEventSeq ?? message.lastEventSeq ?? FALLBACK_EVENT_SEQ;
        const eventSeqEnd = message.lastEventSeq ?? eventSeqStart;
        items.push({
          id: `assistant_text:${message.id}`,
          type: "assistant_text",
          turnId: message.turnId,
          eventSeqStart,
          eventSeqEnd,
          timestamp: message.timestamp,
          stableIndex: stableIndex++,
          payload: { message }
        });
      }
      continue;
    }

    if (message.role === "user") {
      const eventSeqStart = message.startEventSeq ?? message.lastEventSeq ?? FALLBACK_EVENT_SEQ;
      const eventSeqEnd = message.lastEventSeq ?? eventSeqStart;
      items.push({
        id: `user_text:${message.id}`,
        type: "user_text",
        turnId: message.turnId,
        eventSeqStart,
        eventSeqEnd,
        timestamp: message.timestamp,
        stableIndex: stableIndex++,
        payload: { message }
      });
      continue;
    }

    if (message.role === "error") {
      const eventSeqStart = message.startEventSeq ?? message.lastEventSeq ?? FALLBACK_EVENT_SEQ;
      const eventSeqEnd = message.lastEventSeq ?? eventSeqStart;
      items.push({
        id: `error_text:${message.id}`,
        type: "error_text",
        turnId: message.turnId,
        eventSeqStart,
        eventSeqEnd,
        timestamp: message.timestamp,
        stableIndex: stableIndex++,
        payload: { message }
      });
      continue;
    }
  }

  return items.sort((left, right) => compareTimelineItem(left, right));
}

function compareTimelineItem(left: TimelineItem, right: TimelineItem): number {
  if (left.eventSeqStart !== right.eventSeqStart) {
    return left.eventSeqStart - right.eventSeqStart;
  }
  if (left.eventSeqEnd !== right.eventSeqEnd) {
    return left.eventSeqEnd - right.eventSeqEnd;
  }
  const leftMs = parseTimestampToMs(left.timestamp);
  const rightMs = parseTimestampToMs(right.timestamp);
  if (leftMs !== rightMs) {
    return leftMs - rightMs;
  }
  return left.stableIndex - right.stableIndex;
}

function parseTimestampToMs(timestamp: string): number {
  const ms = Date.parse(timestamp);
  return Number.isNaN(ms) ? Number.MAX_SAFE_INTEGER : ms;
}
