import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import MessageList from "./MessageList";
import type { ChatMessage } from "../types/chat";

describe("MessageList", () => {
  it("renders welcome state when empty", () => {
    render(<MessageList messages={[]} pendingApprovals={[]} onResolveApproval={() => {}} />);
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
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
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
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.getByTestId("tool-step-list")).toBeInTheDocument();
    expect(screen.getByText("shell")).toBeInTheDocument();
  });

  it("renders approval card with actions when pending approval matches", () => {
    const messages: ChatMessage[] = [
      {
        id: "m2",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:00Z",
        toolSteps: [
          {
            id: "s2",
            type: "approval",
            title: "approval",
            status: "waiting",
            time: "2026-01-01T00:00:00Z",
            approvalId: "approval_1"
          }
        ]
      }
    ];
    render(
      <MessageList
        messages={messages}
        pendingApprovals={[{ approvalId: "approval_1", toolName: "shell" }]}
        onResolveApproval={() => {}}
      />
    );
    expect(screen.getByTestId("approval-step-card")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "允许" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "拒绝" })).toBeInTheDocument();
  });

  it("matches pending approval by toolCallId when approvalId is absent", () => {
    const messages: ChatMessage[] = [
      {
        id: "m3",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:00Z",
        toolSteps: [
          {
            id: "s3",
            type: "tool",
            title: "shell",
            status: "waiting",
            time: "2026-01-01T00:00:00Z",
            toolCallId: "call_1"
          }
        ]
      }
    ];
    render(
      <MessageList
        messages={messages}
        pendingApprovals={[{ approvalId: "approval_3", toolCallId: "call_1", toolName: "shell" }]}
        onResolveApproval={() => {}}
      />
    );
    expect(screen.getByTestId("approval-step-card")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "允许" })).toBeInTheDocument();
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

    render(<MessageList messages={oldShape} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.queryByTestId("tool-step-list")).not.toBeInTheDocument();
    expect(screen.queryByText("legacy")).not.toBeInTheDocument();
  });

  it("scrolls message container to bottom on new messages", () => {
    const originalClosest = HTMLElement.prototype.closest;
    const scrollContainer = document.createElement("section");
    Object.defineProperty(scrollContainer, "scrollHeight", { value: 999, configurable: true });
    Object.defineProperty(scrollContainer, "clientHeight", { value: 999, configurable: true });
    scrollContainer.scrollTop = 0;
    const closestSpy = vi.spyOn(HTMLElement.prototype, "closest").mockImplementation(function (
      this: HTMLElement,
      selector
    ) {
      if (selector === ".chat-scroll-region") {
        return scrollContainer;
      }
      return originalClosest.call(this, selector);
    });
    const { rerender } = render(
      <MessageList
        messages={[
          {
            id: "m1",
            kind: "text",
            role: "assistant",
            content: "hello",
            timestamp: "2026-01-01T00:00:00Z"
          }
        ]}
        pendingApprovals={[]}
        onResolveApproval={() => {}}
      />
    );

    rerender(
      <MessageList
        messages={[
          {
            id: "m1",
            kind: "text",
            role: "assistant",
            content: "hello",
            timestamp: "2026-01-01T00:00:00Z"
          },
          {
            id: "m2",
            kind: "text",
            role: "assistant",
            content: "world",
            timestamp: "2026-01-01T00:00:01Z"
          }
        ]}
        pendingApprovals={[]}
        onResolveApproval={() => {}}
      />
    );

    expect(scrollContainer.scrollTop).toBe(999);
    closestSpy.mockRestore();
  });
});
