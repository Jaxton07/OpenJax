import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import MessageList from "./MessageList";
import type { ChatMessage } from "../types/chat";

describe("MessageList", () => {
  it("renders welcome state when empty", () => {
    render(<MessageList messages={[]} />);
    expect(screen.getByText("你好，准备好开始了吗？")).toBeInTheDocument();
  });

  it("renders text messages with legacy bubble path", () => {
    const messages: ChatMessage[] = [
      {
        id: "m1",
        kind: "text",
        role: "assistant",
        content: "hello",
        timestamp: "2026-01-01T00:00:00Z"
      }
    ];
    render(<MessageList messages={messages} />);
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("renders tool steps when kind is tool_steps", () => {
    const messages: ChatMessage[] = [
      {
        id: "m1",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:00Z",
        toolSteps: [
          {
            id: "s1",
            type: "tool",
            title: "shell",
            status: "running",
            time: "2026-01-01T00:00:00Z"
          }
        ]
      }
    ];
    render(<MessageList messages={messages} />);
    expect(screen.getByTestId("tool-step-list")).toBeInTheDocument();
    expect(screen.getByText("shell")).toBeInTheDocument();
  });

  it("does not render old assistant+toolSteps shape without kind", () => {
    const oldShape = [
      {
        id: "m1",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:00Z",
        toolSteps: [
          {
            id: "s1",
            type: "tool",
            title: "legacy",
            status: "running",
            time: "2026-01-01T00:00:00Z"
          }
        ]
      }
    ] as unknown as ChatMessage[];

    render(<MessageList messages={oldShape} />);
    expect(screen.queryByTestId("tool-step-list")).not.toBeInTheDocument();
    expect(screen.queryByText("legacy")).not.toBeInTheDocument();
  });
});
