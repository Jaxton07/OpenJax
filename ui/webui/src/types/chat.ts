export interface UserMessage {
  id: string;
  role: "user";
  content: string;
  timestamp: string;
}

export interface AssistantMessage {
  id: string;
  role: "assistant";
  content: string;
  timestamp: string;
  turnId?: string;
}

export interface StreamStoreSnapshot {
  turnId?: string;
  content: string;
  lastEventSeq: number;
  isActive: boolean;
  version: number;
}
