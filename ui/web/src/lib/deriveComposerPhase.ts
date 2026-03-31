import type { ChatSession, ChatMessage } from "../types/chat";

export type ComposerPhase = "idle" | "thinking" | "working";

function findLastDraftMessage(messages: ChatMessage[]): ChatMessage | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message?.isDraft) {
      return message;
    }
  }
  return null;
}

export function deriveComposerPhase(session: ChatSession | null): ComposerPhase {
  if (!session) return "idle";
  const { turnPhase } = session;
  if (turnPhase !== "submitting" && turnPhase !== "streaming") return "idle";

  const draftMessage = findLastDraftMessage(session.messages);

  if (draftMessage?.reasoningBlocks?.some((block) => !block.closed)) return "thinking";
  if (draftMessage?.content && draftMessage.content.length > 0) return "idle";
  return "working";
}
