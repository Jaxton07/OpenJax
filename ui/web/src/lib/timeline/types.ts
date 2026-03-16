import type { ChatMessage, ReasoningBlock, ToolStep } from "../../types/chat";

export type TimelineItemType = "user_text" | "assistant_text" | "reasoning_block" | "tool_step" | "error_text";

interface TimelineItemBase {
  id: string;
  type: TimelineItemType;
  turnId?: string;
  eventSeqStart: number;
  eventSeqEnd: number;
  timestamp: string;
  stableIndex: number;
}

export interface UserTextTimelineItem extends TimelineItemBase {
  type: "user_text";
  payload: { message: ChatMessage };
}

export interface AssistantTextTimelineItem extends TimelineItemBase {
  type: "assistant_text";
  payload: { message: ChatMessage };
}

export interface ErrorTextTimelineItem extends TimelineItemBase {
  type: "error_text";
  payload: { message: ChatMessage };
}

export interface ReasoningBlockTimelineItem extends TimelineItemBase {
  type: "reasoning_block";
  payload: {
    block: ReasoningBlock;
    message: ChatMessage;
    sequenceNumber: number;
  };
}

export interface ToolStepTimelineItem extends TimelineItemBase {
  type: "tool_step";
  payload: {
    step: ToolStep;
    message: ChatMessage;
  };
}

export type TimelineItem =
  | UserTextTimelineItem
  | AssistantTextTimelineItem
  | ErrorTextTimelineItem
  | ReasoningBlockTimelineItem
  | ToolStepTimelineItem;
