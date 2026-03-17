import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ReasoningBlockCard from "./ReasoningBlockCard";
import type { ReasoningBlock } from "../types/chat";

vi.mock("markstream-react", () => ({
  default: ({ content, final }: { content: string; final?: boolean }) => (
    <div data-testid="reasoning-markdown" data-final={String(Boolean(final))}>
      {content}
    </div>
  )
}));

function buildBlock(overrides: Partial<ReasoningBlock> = {}): ReasoningBlock {
  return {
    blockId: "reasoning:turn_1:1",
    turnId: "turn_1",
    content: "## Thinking",
    collapsed: false,
    startedAt: "2026-01-01T00:00:00Z",
    closed: false,
    ...overrides
  };
}

describe("ReasoningBlockCard", () => {
  it("passes final=false when reasoning block is still open", () => {
    render(<ReasoningBlockCard block={buildBlock({ closed: false })} />);
    expect(screen.getByTestId("reasoning-markdown")).toHaveAttribute("data-final", "false");
  });

  it("passes final=true when reasoning block is closed", () => {
    render(<ReasoningBlockCard block={buildBlock({ closed: true, endedAt: "2026-01-01T00:00:02Z" })} />);
    expect(screen.getByTestId("reasoning-markdown")).toHaveAttribute("data-final", "true");
  });
});
