# WebUI Status Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在输入框顶部（新建对话按钮右侧）添加状态动画指示器，Thinking 阶段显示脉冲星云动画，Working 阶段显示矩阵代码雨动画。

**Architecture:** 将阶段判断逻辑提取为纯函数 `deriveComposerPhase`（可独立测试），在 `App.tsx` 调用后将 `streamPhase` 向下传入 `Composer`，由新建的 `StatusIndicator` 组件负责渲染动画。动画样式分文件存放于 `ui/web/src/styles/animations/`。实现需兼容当前 ES2022 配置，不使用 `findLast`。

**Tech Stack:** React 18, TypeScript, CSS animations (keyframes), Vitest

---

## File Map

| 操作 | 路径 | 职责 |
|---|---|---|
| Create | `ui/web/src/lib/deriveComposerPhase.ts` | 纯函数：从 ChatSession 推导阶段，可单独测试 |
| Create | `ui/web/src/lib/__tests__/deriveComposerPhase.test.ts` | 单元测试 |
| Create | `ui/web/src/styles/animations/thinking.css` | cyber-liquid-4 动画定义 |
| Create | `ui/web/src/styles/animations/working.css` | cyber-matrix 动画定义 |
| Create | `ui/web/src/components/composer/StatusIndicator.tsx` | 状态指示器 React 组件 |
| Create | `ui/web/src/components/composer/status-indicator.css` | StatusIndicator 布局样式 |
| Modify | `ui/web/src/components/composer/index.tsx` | 接收 `streamPhase` prop，渲染 StatusIndicator |
| Modify | `ui/web/src/components/composer/composer.css` | 新增 `.composer-actions-bar` flex 行 |
| Modify | `ui/web/src/App.tsx` | 调用 `deriveComposerPhase`，传入 `Composer` |

---

## Task 1: 阶段推导纯函数 + 单元测试

**Files:**
- Create: `ui/web/src/lib/deriveComposerPhase.ts`
- Create: `ui/web/src/lib/__tests__/deriveComposerPhase.test.ts`

- [ ] **Step 1: 新建测试文件（TDD：先写测试）**

```typescript
// ui/web/src/lib/__tests__/deriveComposerPhase.test.ts
import { describe, expect, it } from "vitest";
import type { ChatSession } from "../../types/chat";
import { deriveComposerPhase } from "../deriveComposerPhase";

function baseSession(overrides: Partial<ChatSession> = {}): ChatSession {
  return {
    id: "sess_1",
    title: "test",
    isPlaceholderTitle: false,
    createdAt: "2026-01-01T00:00:00Z",
    connection: "active",
    turnPhase: "draft",
    lastEventSeq: 0,
    messages: [],
    pendingApprovals: [],
    ...overrides,
  };
}

describe("deriveComposerPhase", () => {
  it("returns idle when session is null", () => {
    expect(deriveComposerPhase(null)).toBe("idle");
  });

  it("returns idle when turnPhase is draft", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "draft" }))).toBe("idle");
  });

  it("returns idle when turnPhase is completed", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "completed" }))).toBe("idle");
  });

  it("returns working when turnPhase is submitting", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "submitting" }))).toBe("working");
  });

  it("returns working when streaming with no draft message", () => {
    expect(deriveComposerPhase(baseSession({ turnPhase: "streaming" }))).toBe("working");
  });

  it("returns thinking when streaming draft has an open reasoning block", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          reasoningBlocks: [
            {
              blockId: "rb_1",
              turnId: "turn_1",
              content: "hmm...",
              collapsed: false,
              startedAt: "2026-01-01T00:00:00Z",
              closed: false,
            },
          ],
        },
      ],
    });
    expect(deriveComposerPhase(session)).toBe("thinking");
  });

  it("returns working when streaming draft has only closed reasoning blocks and no content", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          reasoningBlocks: [
            {
              blockId: "rb_1",
              turnId: "turn_1",
              content: "done thinking",
              collapsed: false,
              startedAt: "2026-01-01T00:00:00Z",
              closed: true,
            },
          ],
        },
      ],
    });
    expect(deriveComposerPhase(session)).toBe("working");
  });

  it("returns idle when streaming draft has text content (outputting text)", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "text",
          role: "assistant",
          content: "Here is my answer...",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
        },
      ],
    });
    expect(deriveComposerPhase(session)).toBe("idle");
  });

  it("returns working when streaming with running tool steps", () => {
    const session = baseSession({
      turnPhase: "streaming",
      messages: [
        {
          id: "msg_1",
          kind: "tool_steps",
          role: "tool",
          content: "",
          timestamp: "2026-01-01T00:00:00Z",
          isDraft: true,
          toolSteps: [
            {
              id: "step_1",
              type: "tool",
              title: "bash",
              status: "running",
              time: "2026-01-01T00:00:00Z",
            },
          ],
        },
      ],
    });
    expect(deriveComposerPhase(session)).toBe("working");
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cd ui/web && pnpm test -- deriveComposerPhase
```

Expected: FAIL — `Cannot find module '../deriveComposerPhase'`

- [ ] **Step 3: 实现 deriveComposerPhase**

```typescript
// ui/web/src/lib/deriveComposerPhase.ts
import type { ChatSession } from "../types/chat";

export type ComposerPhase = "idle" | "thinking" | "working";

/**
 * 从当前会话状态推导 Composer 状态指示器应显示的阶段。
 *
 * - thinking: 正在推理（有未关闭的 reasoning block）
 * - working:  忙碌中但不在推理也不在输出正文（submitting、tool call、阶段间隙等）
 * - idle:     未忙碌，或正在输出正文（此时不显示指示器）
 */
export function deriveComposerPhase(session: ChatSession | null): ComposerPhase {
  if (!session) return "idle";
  const { turnPhase } = session;
  if (turnPhase !== "submitting" && turnPhase !== "streaming") return "idle";

  let draftMsg = undefined;
  for (let i = session.messages.length - 1; i >= 0; i -= 1) {
    if (session.messages[i]?.isDraft) {
      draftMsg = session.messages[i];
      break;
    }
  }

  // 有未关闭的 reasoning block → thinking
  if (draftMsg?.reasoningBlocks?.some((b) => !b.closed)) return "thinking";

  // 正在输出正文 → 不显示指示器
  if (draftMsg?.content && draftMsg.content.length > 0) return "idle";

  // 其余忙碌状态（submitting、tool call 执行中、阶段间隙）→ working
  return "working";
}
```

- [ ] **Step 4: 运行测试，确认全部通过**

```bash
cd ui/web && pnpm test -- deriveComposerPhase
```

Expected: 9 tests PASS

---

## Task 2: 动画 CSS 文件

**Files:**
- Create: `ui/web/src/styles/animations/thinking.css`
- Create: `ui/web/src/styles/animations/working.css`

- [ ] **Step 1: 创建 Thinking 动画 CSS**

```css
/* ui/web/src/styles/animations/thinking.css */
/* CYBER-04: 脉冲星云 (30px) — Thinking 状态动画 */

@keyframes cyber-nebula {
  0%, 100% {
    border-radius: 70% 30% 50% 50% / 30% 30% 70% 70%;
    transform: rotate(0deg) scale(1);
  }
  25% {
    border-radius: 30% 60% 70% 40% / 50% 40% 60% 50%;
    transform: rotate(90deg) scale(1.1);
  }
  50% {
    border-radius: 50% 60% 30% 60% / 30% 70% 50% 70%;
    transform: rotate(180deg) scale(0.95);
  }
  75% {
    border-radius: 60% 40% 60% 40% / 60% 30% 70% 40%;
    transform: rotate(270deg) scale(1.05);
  }
}

@keyframes gradient-shift {
  0%, 100% { background-position: 0% 50%; }
  50% { background-position: 100% 50%; }
}

.cyber-liquid-4 {
  position: relative;
  width: 30px;
  height: 30px;
  flex-shrink: 0;
}

.cyber-liquid-4::before,
.cyber-liquid-4::after,
.cyber-liquid-4 .inner {
  content: '';
  position: absolute;
  border-radius: 70% 30% 50% 50% / 30% 30% 70% 70%;
  animation: cyber-nebula 4s ease-in-out infinite;
}

.cyber-liquid-4::before {
  inset: -5px;
  background: linear-gradient(45deg, #ff006e, transparent, #8338ec);
  opacity: 0.4;
  filter: blur(4px);
  animation-delay: 0s;
}

.cyber-liquid-4::after {
  inset: -2px;
  background: linear-gradient(135deg, #00f5ff, transparent, #ff00ff);
  opacity: 0.6;
  filter: blur(2px);
  animation-delay: -1.3s;
}

.cyber-liquid-4 .inner {
  inset: 0;
  background: linear-gradient(90deg, #ff006e, #8338ec, #00f5ff);
  background-size: 300% 300%;
  animation:
    cyber-nebula 4s ease-in-out infinite,
    gradient-shift 3s ease infinite;
  animation-delay: -2.6s;
  box-shadow: inset 0 0 10px rgba(255, 255, 255, 0.3);
}
```

- [ ] **Step 2: 创建 Working 动画 CSS**

```css
/* ui/web/src/styles/animations/working.css */
/* 赛博朋克矩阵代码雨 — Working 状态动画 */

@keyframes cyber-fall {
  0%   { transform: translateY(-100%); opacity: 0; filter: blur(4px); }
  30%  { opacity: 1; filter: blur(0px); }
  70%  { opacity: 1; filter: blur(0px); }
  100% { transform: translateY(100%); opacity: 0; filter: blur(4px); }
}

.cyber-matrix {
  display: flex;
  gap: 3px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
  height: 30px;
  overflow: hidden;
  padding: 4px;
  flex-shrink: 0;
}

.cyber-col {
  display: flex;
  flex-direction: column;
  animation: cyber-fall 0.9s linear infinite;
  line-height: 1.2;
}

.cyber-col:nth-child(1) { animation-delay: 0s;    color: #00f5ff; text-shadow: 0 0 8px #00f5ff; }
.cyber-col:nth-child(2) { animation-delay: 0.2s;  color: #ff006e; text-shadow: 0 0 8px #ff006e; }
.cyber-col:nth-child(3) { animation-delay: 0.4s;  color: #8338ec; text-shadow: 0 0 8px #8338ec; }
.cyber-col:nth-child(4) { animation-delay: 0.15s; color: #ff00ff; text-shadow: 0 0 8px #ff00ff; }
.cyber-col:nth-child(5) { animation-delay: 0.35s; color: #3a86ff; text-shadow: 0 0 8px #3a86ff; }
```


---

## Task 3: StatusIndicator 组件

**Files:**
- Create: `ui/web/src/components/composer/StatusIndicator.tsx`
- Create: `ui/web/src/components/composer/status-indicator.css`

- [ ] **Step 1: 创建 status-indicator.css**

```css
/* ui/web/src/components/composer/status-indicator.css */

.status-indicator {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  opacity: 0;
  transform: translateX(-6px);
  transition: opacity 0.2s ease, transform 0.2s ease;
  pointer-events: none;
}

.status-indicator.visible {
  opacity: 1;
  transform: translateX(0);
}

.status-indicator-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--ink-3);
  white-space: nowrap;
  letter-spacing: 0.02em;
}
```

- [ ] **Step 2: 创建 StatusIndicator.tsx**

```tsx
// ui/web/src/components/composer/StatusIndicator.tsx
import "../../styles/animations/thinking.css";
import "../../styles/animations/working.css";
import "./status-indicator.css";
import type { ComposerPhase } from "../../lib/deriveComposerPhase";

interface StatusIndicatorProps {
  phase: ComposerPhase;
}

export default function StatusIndicator({ phase }: StatusIndicatorProps) {
  const visible = phase !== "idle";

  return (
    <div className={`status-indicator${visible ? " visible" : ""}`} aria-live="polite" aria-atomic="true">
      {phase === "thinking" && (
        <>
          <div className="cyber-liquid-4">
            <div className="inner" />
          </div>
          <span className="status-indicator-label">Thinking</span>
        </>
      )}
      {phase === "working" && (
        <>
          <div className="cyber-matrix">
            <div className="cyber-col"><span>0</span><span>1</span><span>0</span></div>
            <div className="cyber-col"><span>1</span><span>0</span><span>1</span></div>
            <div className="cyber-col"><span>0</span><span>1</span><span>0</span></div>
            <div className="cyber-col"><span>1</span><span>0</span><span>1</span></div>
            <div className="cyber-col"><span>0</span><span>1</span><span>0</span></div>
          </div>
          <span className="status-indicator-label">Working</span>
        </>
      )}
    </div>
  );
}
```


---

## Task 4: 接入 Composer 和 App

**Files:**
- Modify: `ui/web/src/components/composer/composer.css`
- Modify: `ui/web/src/components/composer/index.tsx`
- Modify: `ui/web/src/App.tsx`

- [ ] **Step 1: 在 composer.css 中添加 actions bar 布局**

在 `composer.css` 文件中，找到 `.composer-actions` 规则块（约第 11 行），在其**前面**插入以下内容：

```css
.composer-actions-bar {
  display: flex;
  align-items: center;
  gap: 0;
  margin-bottom: 8px;
}
```

同时将原有 `.composer-actions` 的 `margin-bottom: 8px;` 删除（该外边距已移至 `.composer-actions-bar`）：

```css
/* 修改后的 .composer-actions */
.composer-actions {
  display: inline-flex;
  gap: 8px;
  width: fit-content;
}
```

- [ ] **Step 2: 修改 composer/index.tsx，接收 streamPhase 并渲染 StatusIndicator**

完整替换文件内容：

```tsx
// ui/web/src/components/composer/index.tsx
import { useLayoutEffect, useRef, useState } from "react";
import "./composer.css";
import ComposerActions from "./ComposerActions";
import ComposerInput from "./ComposerInput";
import SlashDropdown from "./SlashDropdown";
import StatusIndicator from "./StatusIndicator";
import { useSlashCommands } from "../../hooks/useSlashCommands";
import type { ComposerPhase } from "../../lib/deriveComposerPhase";
import type { ContextUsageState } from "../../types/chat";
import type { SlashCommandDto } from "../../types/gateway";

interface ComposerProps {
  disabled?: boolean;
  baseUrl: string;
  accessToken: string;
  sessionId?: string | null;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onClear?: () => Promise<void> | void;
  contextUsage?: ContextUsageState | null;
  policyLevel?: "allow" | "ask" | "deny";
  onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
  isBusyTurn?: boolean;
  isStreaming?: boolean;
  streamPhase?: ComposerPhase;
  onBlockedSendAttempt?: () => void;
  onStop?: () => void;
}

export default function Composer({
  disabled,
  baseUrl,
  accessToken,
  sessionId,
  onSend,
  onNewChat,
  onClear,
  contextUsage,
  policyLevel,
  onPolicyLevelChange,
  isBusyTurn,
  isStreaming,
  streamPhase = "idle",
  onBlockedSendAttempt,
  onStop
}: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { commands, filterCommands, findCommand } = useSlashCommands(baseUrl, accessToken);

  const [showSlashDropdown, setShowSlashDropdown] = useState(false);
  const [slashSelectedIndex, setSlashSelectedIndex] = useState(0);
  const [slashMatches, setSlashMatches] = useState<SlashCommandDto[]>([]);

  const resizeTextarea = () => {
    const el = textareaRef.current;
    if (!el) {
      return;
    }
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
  };

  useLayoutEffect(() => {
    resizeTextarea();
  }, [input]);

  // Detect slash command in input and update dropdown
  useLayoutEffect(() => {
    const slashMatch = input.match(/^\/(\w*)$/);
    if (slashMatch) {
      const query = slashMatch[0];
      const matches = filterCommands(query);
      setSlashMatches(matches);
      setShowSlashDropdown(matches.length > 0);
      setSlashSelectedIndex(0);
    } else {
      setShowSlashDropdown(false);
    }
  }, [input, filterCommands]);

  const resetComposer = () => {
    setInput("");
    const el = textareaRef.current;
    if (el) el.style.height = "";
    setShowSlashDropdown(false);
  };

  const submitResolvedCommand = async (command: SlashCommandDto) => {
    const cmdName = command.name.toLowerCase();

    if (cmdName === "policy") {
      resetComposer();
      return;
    }

    if (cmdName === "clear") {
      resetComposer();
      await onClear?.();
      return;
    }

    if (command.kind === "session_action") {
      if (!sessionId) {
        resetComposer();
        return;
      }
      try {
        await fetch(`${baseUrl}/api/v1/sessions/${sessionId}/slash`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${accessToken}`,
          },
          body: JSON.stringify({ command: cmdName }),
        });
      } catch {
        // 网络失败时静默忽略，避免 UI 卡住
      }
      resetComposer();
      return;
    }

    if (command.kind === "local_picker") {
      resetComposer();
      return;
    }

    resetComposer();
    try {
      await onSend(`/${cmdName}`);
    } catch {
      // 忽略
    }
  };

  const submit = async () => {
    if (isBusyTurn) {
      onBlockedSendAttempt?.();
      return;
    }
    const trimmed = input.trim();
    if (!trimmed) return;

    const slashMatch = trimmed.match(/^\/([\w]+)$/);
    if (slashMatch) {
      const matched = findCommand(trimmed);
      if (matched) {
        await submitResolvedCommand(matched);
        return;
      }
    }

    const content = trimmed;
    resetComposer();
    try {
      await onSend(content);
    } catch {
      // 忽略
    }
  };

  const handleSlashClose = () => {
    setShowSlashDropdown(false);
  };

  const handleSlashSelect = (cmd: SlashCommandDto) => {
    setInput(`/${cmd.name} `);
    setShowSlashDropdown(false);
    textareaRef.current?.focus();
  };

  const handleSlashSubmit = async () => {
    if (isBusyTurn) {
      onBlockedSendAttempt?.();
      return;
    }
    const matched = slashMatches[effectiveSlashIndex];
    if (!matched) {
      await submit();
      return;
    }
    await submitResolvedCommand(matched);
  };

  // Clamp selected index to valid range when dropdown is visible
  const effectiveSlashIndex =
    slashSelectedIndex < 0
      ? slashMatches.length - 1
      : slashSelectedIndex >= slashMatches.length
        ? 0
        : slashSelectedIndex;

  return (
    <div className="composer-wrap">
      <div className="composer-actions-bar">
        <ComposerActions onNewChat={onNewChat} />
        <StatusIndicator phase={streamPhase} />
      </div>
      <SlashDropdown
        visible={showSlashDropdown}
        commands={slashMatches}
        selectedIndex={effectiveSlashIndex}
        onSelect={handleSlashSelect}
      />
      <ComposerInput
        value={input}
        disabled={disabled}
        textareaRef={textareaRef}
        onChange={setInput}
        onSubmit={() => void submit()}
        contextUsage={contextUsage}
        showSlashDropdown={showSlashDropdown}
        slashSelectedIndex={effectiveSlashIndex}
        onSlashIndexChange={(i) => setSlashSelectedIndex(i)}
        onSlashClose={handleSlashClose}
        onSlashSubmit={() => void handleSlashSubmit()}
        policyLevel={policyLevel}
        onPolicyLevelChange={onPolicyLevelChange}
        isBusyTurn={isBusyTurn}
        isStreaming={isStreaming}
        onStop={onStop}
      />
    </div>
  );
}
```

- [ ] **Step 3: 修改 App.tsx，派生 streamPhase 并传入 Composer**

在 `App.tsx` 中，在 `useChatApp` 解构后（约第 66 行，`} = useChatApp();` 之后）添加以下 import 和 useMemo：

在文件顶部 import 区添加：
```tsx
import { useMemo } from "react";  // 已有 useEffect/useState，加入 useMemo
import { deriveComposerPhase } from "./lib/deriveComposerPhase";
```

注意：`useMemo` 已在第 1 行 `import { useEffect, useMemo, useState } from "react"` 中导入，无需重复。只需添加 `deriveComposerPhase` 的 import。

在 `} = useChatApp();` 后新增：

```tsx
const composerPhase = useMemo(
  () => deriveComposerPhase(activeSession),
  [activeSession]
);
```

在 `<Composer` JSX 中添加 `streamPhase` prop（紧跟 `isStreaming` 之后）：

```tsx
<Composer
  disabled={state.loading}
  baseUrl={state.settings.baseUrl}
  accessToken={state.auth.accessToken}
  sessionId={activeSession?.id}
  contextUsage={activeSession?.contextUsage ?? null}
  onSend={sendMessage}
  onNewChat={() => void newChat()}
  onClear={() => void clearConversation()}
  policyLevel={activeSession?.policyLevel ?? draftPolicyLevel}
  onPolicyLevelChange={onPolicyLevelChange}
  isBusyTurn={isBusyTurn}
  isStreaming={isStreaming}
  streamPhase={composerPhase}
  onBlockedSendAttempt={notifyBusyTurnBlockedSend}
  onStop={() => void abortTurn()}
/>
```

- [ ] **Step 4: 构建验证无类型错误**

```bash
cd ui/web && pnpm build
```

Expected: Build 成功，无 TypeScript 错误

- [ ] **Step 5: 运行全量测试**

```bash
cd ui/web && pnpm test
```

Expected: 所有测试通过（包括新增的 9 个 deriveComposerPhase 测试）


---

## 验收检查

启动开发环境后（`make run-web-dev`，访问 `http://127.0.0.1:<port>`）：

1. **Working**：发送一条消息，提交瞬间新建对话按钮右侧出现矩阵代码雨 + "Working" 文字
2. **Thinking**：对接支持 reasoning 的模型（如 Kimi），发送消息后 reasoning 阶段出现脉冲星云 + "Thinking" 文字
3. **文字输出**：模型开始输出正文时，动画消失
4. **完成后**：`turnPhase` 变为 `completed`，动画消失
5. **无回归**：新建对话、slash 命令、stop 按钮功能正常
