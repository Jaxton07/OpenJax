import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { streamRenderStore } from "../lib/streamRenderStore";
import MessageList from "./MessageList";
import type { ChatMessage } from "../types/chat";

describe("MessageList", () => {
  afterEach(() => {
    delete (globalThis as { OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string }).OPENJAX_WEB_ASSISTANT_RENDER_MODE;
    streamRenderStore.__dangerousResetForTests();
  });

  it("renders welcome state when empty", () => {
    render(<MessageList messages={[]} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.getByText("你好，准备好开始了吗？")).toBeInTheDocument();
  });

  it("renders assistant markdown content", () => {
    (globalThis as { OPENJAX_WEB_ASSISTANT_RENDER_MODE?: string }).OPENJAX_WEB_ASSISTANT_RENDER_MODE =
      "markdown";
    const messages: ChatMessage[] = [
      {
        id: "m1",
        kind: "text",
        role: "assistant",
        content: "## Hello\n\nThis is **bold**.\n\n```ts\nconst x = 1\n```",
        timestamp: "2026-01-01T00:00:00Z"
      }
    ];
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.getByText("Hello")).toBeInTheDocument();
    expect(screen.getByText("bold")).toBeInTheDocument();
    expect(screen.getByText("const x = 1")).toBeInTheDocument();
  });

  it("renders assistant text mode by default", () => {
    const messages: ChatMessage[] = [
      {
        id: "m1",
        kind: "text",
        role: "assistant",
        content: "## Hello\n\nThis is **bold**.",
        timestamp: "2026-01-01T00:00:00Z"
      }
    ];
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.getByText((content) => content.includes("## Hello") && content.includes("**bold**"))).toBeInTheDocument();
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

  it("prefers runtime streaming content for assistant draft", () => {
    const messages: ChatMessage[] = [
      {
        id: "m1",
        kind: "text",
        role: "assistant",
        content: "你有什",
        timestamp: "2026-01-01T00:00:00Z",
        turnId: "turn_1",
        isDraft: true
      }
    ];
    act(() => {
      streamRenderStore.start("sess_1", "turn_1", "m1", 2, "你好！有什么我可以帮您的吗？");
    });
    render(
      <MessageList
        sessionId="sess_1"
        messages={messages}
        pendingApprovals={[]}
        onResolveApproval={() => {}}
      />
    );
    expect(screen.getByText("你好！有什么我可以帮您的吗？")).toBeInTheDocument();
  });

  it("renders reasoning block as standalone timeline item and keeps collapsed by default", async () => {
    const user = userEvent.setup();
    const messages: ChatMessage[] = [
      {
        id: "m_reason",
        kind: "text",
        role: "assistant",
        content: "这是最终正文",
        timestamp: "2026-01-01T00:00:00Z",
        turnId: "turn_1",
        startEventSeq: 3,
        lastEventSeq: 3,
        reasoningBlocks: [
          {
            blockId: "reasoning:turn_1:1",
            turnId: "turn_1",
            content: "先分析问题",
            collapsed: true,
            startedAt: "2026-01-01T00:00:00Z",
            closed: true,
            startEventSeq: 1,
            lastEventSeq: 1,
            endEventSeq: 1
          }
        ]
      }
    ];
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
    const toggle = screen.getByRole("button", { name: /思考过程 1/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    const toggleNode = toggle.closest(".reasoning-block");
    expect(toggleNode).not.toBeNull();
    const contentNode = screen.getByText("这是最终正文");
    expect(toggleNode!.compareDocumentPosition(contentNode) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("先分析问题")).toBeInTheDocument();
  });

  it("renders multiple reasoning blocks in order", () => {
    const messages: ChatMessage[] = [
      {
        id: "m_reason_2",
        kind: "text",
        role: "assistant",
        content: "正文",
        timestamp: "2026-01-01T00:00:00Z",
        turnId: "turn_2",
        reasoningBlocks: [
          {
            blockId: "reasoning:turn_2:1",
            turnId: "turn_2",
            content: "第一段",
            collapsed: true,
            startedAt: "2026-01-01T00:00:00Z",
            closed: true,
            startEventSeq: 1,
            lastEventSeq: 1,
            endEventSeq: 1
          },
          {
            blockId: "reasoning:turn_2:2",
            turnId: "turn_2",
            content: "第二段",
            collapsed: true,
            startedAt: "2026-01-01T00:00:01Z",
            closed: false,
            startEventSeq: 2,
            lastEventSeq: 2
          }
        ]
      }
    ];
    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);
    expect(screen.getByRole("button", { name: /思考过程 1/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /思考过程 2/ })).toBeInTheDocument();
  });

  it("renders timeline by event_seq order", () => {
    const messages: ChatMessage[] = [
      {
        id: "assistant_late",
        kind: "text",
        role: "assistant",
        content: "最终回答",
        timestamp: "2026-01-01T00:00:01Z",
        turnId: "turn_1",
        startEventSeq: 13,
        lastEventSeq: 13
      },
      {
        id: "tool_mid",
        kind: "tool_steps",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:10Z",
        turnId: "turn_1",
        toolSteps: [
          {
            id: "step_1",
            type: "tool",
            title: "read_file",
            status: "success",
            time: "2026-01-01T00:00:10Z",
            startEventSeq: 11,
            lastEventSeq: 11,
            endEventSeq: 11
          }
        ]
      },
      {
        id: "assistant_reason",
        kind: "text",
        role: "assistant",
        content: "",
        timestamp: "2026-01-01T00:00:09Z",
        turnId: "turn_1",
        reasoningBlocks: [
          {
            blockId: "reasoning:turn_1:10",
            turnId: "turn_1",
            content: "先思考",
            collapsed: true,
            startedAt: "2026-01-01T00:00:09Z",
            closed: true,
            startEventSeq: 10,
            lastEventSeq: 10,
            endEventSeq: 10
          },
          {
            blockId: "reasoning:turn_1:12",
            turnId: "turn_1",
            content: "再思考",
            collapsed: true,
            startedAt: "2026-01-01T00:00:11Z",
            closed: false,
            startEventSeq: 12,
            lastEventSeq: 12
          }
        ]
      },
      {
        id: "user_early",
        kind: "text",
        role: "user",
        content: "请读取 test.txt",
        timestamp: "2026-01-01T00:00:02Z",
        startEventSeq: 1,
        lastEventSeq: 1
      }
    ];

    render(<MessageList messages={messages} pendingApprovals={[]} onResolveApproval={() => {}} />);

    const userNode = screen.getByText("请读取 test.txt");
    const reasoning1 = screen.getByRole("button", { name: /思考过程 1/ });
    const toolNode = screen.getByText("read_file");
    const reasoning2 = screen.getByRole("button", { name: /思考过程 2/ });
    const assistantNode = screen.getByText("最终回答");

    expect(userNode.compareDocumentPosition(reasoning1) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(reasoning1.compareDocumentPosition(toolNode) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(toolNode.compareDocumentPosition(reasoning2) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(reasoning2.compareDocumentPosition(assistantNode) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
