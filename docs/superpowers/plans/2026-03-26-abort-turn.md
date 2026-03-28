# Agent 任务中断功能 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户能在任务执行中途中断 Agent，TUI 用 Esc 键，Web UI 用 Stop 按钮替换发送按钮。

**Architecture:** 使用 `JoinHandle::abort()` 从外部强制终止 tokio 任务，不修改 openjax-core。Gateway 侧将 `AbortHandle` 存入 `SessionRuntime`，通过新的 `:abort` session action 触发；abort 后广播合成的 `turn_interrupted` SSE 事件。TUI 侧在 Esc（无 overlay）时直接 abort `turn_task`，补发合成 `TurnCompleted`。

**Tech Stack:** Rust (tokio, axum), TypeScript (React, vitest)

**Spec:** `docs/superpowers/specs/2026-03-26-abort-turn-design.md`

---

## File Map

| 文件 | 操作 |
|------|------|
| `openjax-gateway/src/state/runtime.rs` | 修改：`SessionRuntime` 增加 `current_turn_abort_handle` 字段 |
| `openjax-gateway/src/state/events.rs` | 修改：`run_turn_task` 存储 abort handle，处理 cancelled 错误 |
| `openjax-gateway/src/handlers/session.rs` | 修改：`session_action` 增加 `"abort"` 分支 |
| `ui/tui/src/runtime.rs` | 修改：`dismiss_overlay` 增加 abort turn 分支 |
| `ui/web/src/types/gateway.ts` | 修改：`StreamEvent.type` 增加 `"turn_interrupted"` |
| `ui/web/src/types/chat.ts` | 修改：`ChatMessage` 增加 `interrupted?: boolean` |
| `ui/web/src/pic/icon/index.tsx` | 修改：增加 `StopCircleIcon` 内联组件 |
| `ui/web/src/lib/gatewayClient.ts` | 修改：增加 `abortTurn(sessionId)` 方法 |
| `ui/web/src/hooks/useChatApp.ts` | 修改：增加 `isStreaming` 计算值和 `abortTurn` action |
| `ui/web/src/components/composer/ComposerInput.tsx` | 修改：流式时渲染 Stop 按钮 |
| `ui/web/src/components/composer/index.tsx` | 修改：接收并透传 `isStreaming` / `onStop` |
| `ui/web/src/App.tsx` | 修改：向 Composer 传 `isStreaming` / `onStop` |
| `ui/web/src/lib/session-events/reducer.ts` | 修改：处理 `turn_interrupted` 事件 |
| `ui/web/src/components/MessageList.tsx` | 修改：已中断消息末尾显示标记 |

---

## Task 1: Gateway — SessionRuntime 增加 abort handle 字段

**Files:**
- Modify: `openjax-gateway/src/state/runtime.rs`

- [ ] **Step 1: 在 `SessionRuntime` 结构体中添加字段**

  在 `openjax-gateway/src/state/runtime.rs` 的 `SessionRuntime` 结构体（第 114 行）中，在 `last_event_emitted_at` 字段之后添加：

  ```rust
  pub current_turn_abort_handle: Option<tokio::task::AbortHandle>,
  ```

- [ ] **Step 2: 在 `new_with_config` 中初始化新字段**

  在 `SessionRuntime::new_with_config`（第 132 行）的 `Self { ... }` 块中添加：
  ```rust
  current_turn_abort_handle: None,
  ```

- [ ] **Step 3: 在 `clear_context` 和 `clear_context_with_config` 中重置字段**

  在两个 `clear_context*` 方法中，在 `self.last_event_emitted_at = None;` 后添加：
  ```rust
  self.current_turn_abort_handle = None;
  ```

- [ ] **Step 4: 写单元测试验证初始值**

  在 `openjax-gateway/src/state/runtime.rs` 底部的 `#[cfg(test)]` 块（若无则新建）中添加：
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use openjax_core::Config;

      #[test]
      fn session_runtime_abort_handle_starts_none() {
          let runtime = SessionRuntime::new_with_config(Config::default());
          assert!(runtime.current_turn_abort_handle.is_none());
      }

      #[test]
      fn clear_context_resets_abort_handle() {
          let mut runtime = SessionRuntime::new_with_config(Config::default());
          // 用一个已完成任务的 abort handle 模拟"有值"状态
          let handle = tokio::runtime::Runtime::new()
              .unwrap()
              .spawn(async {})
              .abort_handle();
          runtime.current_turn_abort_handle = Some(handle);
          runtime.clear_context();
          assert!(runtime.current_turn_abort_handle.is_none());
      }
  }
  ```

- [ ] **Step 5: 运行测试验证通过**

  ```bash
  zsh -lc "cargo test -p openjax-gateway -- state::runtime::tests"
  ```
  Expected: 2 tests pass

- [ ] **Step 6: Commit**

  ```bash
  git add openjax-gateway/src/state/runtime.rs
  git commit -m "feat(gateway): SessionRuntime 增加 current_turn_abort_handle 字段"
  ```

---

## Task 2: Gateway — run_turn_task 存储并处理 abort handle

**Files:**
- Modify: `openjax-gateway/src/state/events.rs`

- [ ] **Step 1: 了解 `run_turn_task` 当前结构**

  阅读 `openjax-gateway/src/state/events.rs` 第 349-423 行，理解事件循环结构。

- [ ] **Step 2: 在 spawn 之后立即存储 abort handle**

  在 `let submit_task = tokio::spawn(...)` 块（第 372-379 行）结束后（`};` 之后），添加：
  ```rust
  // 用短锁存储 abort handle，必须在事件循环开始前完成
  {
      let mut session = session_runtime.lock().await;
      session.current_turn_abort_handle = Some(submit_task.abort_handle());
  }
  ```

- [ ] **Step 3: 在事件循环中追踪最后已知的 public turn_id**

  在 `let mut sent_turn_id = false;` 之后添加：
  ```rust
  let mut last_known_public_turn_id: Option<String> = None;
  ```

  在事件循环的 `if mapped.is_some()` 块中追踪：
  ```rust
  if let Some(ref turn_id) = mapped {
      sent_turn_id = true;
      last_known_public_turn_id = Some(turn_id.clone());
  }
  ```
  （替换原来的 `if mapped.is_some() { sent_turn_id = true; }`）

- [ ] **Step 4: 处理 abort（cancelled）错误**

  将 `submit_task.await` 的 `Err(_)` 分支（第 414-421 行）替换为：

  ```rust
  Err(join_error) => {
      // 清除 abort handle
      {
          let mut session = session_runtime.lock().await;
          session.current_turn_abort_handle = None;
      }

      if join_error.is_cancelled() {
          // 广播 turn_interrupted 合成事件
          let mut session = session_runtime.lock().await;
          let public_turn_id = last_known_public_turn_id.clone().or_else(|| {
              // 兜底：取 turns 中最后一个 Running 状态的 turn_id
              session
                  .turns
                  .iter()
                  .find(|(_, t)| t.status == TurnStatus::Running)
                  .map(|(id, _)| id.clone())
          });
          if let Some(ref turn_id) = public_turn_id {
              if let Some(turn) = session.turns.get_mut(turn_id) {
                  turn.status = TurnStatus::Failed;
              }
          }
          let envelope = session.create_gateway_event(
              &request_id,
              &session_id,
              public_turn_id,
              "turn_interrupted",
              json!({ "reason": "user_abort" }),
              Some("synthetic"),
          );
          publish_and_persist_event(&app_state, &mut session, envelope);
      }

      if let Some(tx) = pending_turn_id_tx.take() {
          let _ = tx.send(Err(ApiError::upstream_unavailable(
              "core execution task aborted",
              json!({}),
          )));
      }
  }
  ```

- [ ] **Step 5: 任务正常完成时也清除 abort handle**

  在 `Ok(events)` 分支开头添加：
  ```rust
  Ok(events) => {
      {
          let mut session = session_runtime.lock().await;
          session.current_turn_abort_handle = None;
      }
      // ... 原有逻辑不变
  ```

- [ ] **Step 6: 构建验证编译通过**

  ```bash
  zsh -lc "cargo build -p openjax-gateway"
  ```
  Expected: 编译通过，无错误

- [ ] **Step 7: 运行 Gateway 测试**

  ```bash
  zsh -lc "cargo test -p openjax-gateway"
  ```
  Expected: 所有现有测试通过

- [ ] **Step 8: Commit**

  ```bash
  git add openjax-gateway/src/state/events.rs
  git commit -m "feat(gateway): run_turn_task 存储 abort handle 并处理 cancelled 错误"
  ```

---

## Task 3: Gateway — session_action 增加 abort 分支

**Files:**
- Modify: `openjax-gateway/src/handlers/session.rs`

- [ ] **Step 1: 写失败测试**

  在 `openjax-gateway/src/handlers/session.rs` 末尾的测试模块（或创建新的）中添加：
  ```rust
  #[cfg(test)]
  mod abort_tests {
      use super::{normalize_session_action, parse_session_action};

      #[test]
      fn abort_action_parses_correctly() {
          let (session_id, action) = parse_session_action("sess_abc123:abort").unwrap();
          assert_eq!(session_id, "sess_abc123");
          assert_eq!(normalize_session_action(action), "abort");
      }
  }
  ```

- [ ] **Step 2: 运行测试（此时已通过，因 parse/normalize 已有实现）**

  ```bash
  zsh -lc "cargo test -p openjax-gateway -- abort_tests"
  ```
  Expected: PASS（parse/normalize 逻辑无需改动）

- [ ] **Step 3: 在 `session_action` handler 中添加 abort 分支**

  在 `session_action` 函数中，`compact` 分支之后、`clear` 判断之前，添加：

  ```rust
  if normalized == "abort" {
      let handle = {
          let mut session = session_runtime.lock().await;
          session.current_turn_abort_handle.take()
      };
      if let Some(handle) = handle {
          handle.abort();
      }
      // 无论是否有任务在运行，都返回 200（幂等）
      return Ok(Json(SessionActionResponse {
          request_id: ctx.request_id,
          session_id: session_id.to_string(),
          status: "aborted",
          timestamp: now_rfc3339(),
      }));
  }
  ```

  注意：`SessionActionResponse.status` 字段当前类型是 `&'static str`，`"aborted"` 需加入有效值。检查该类型定义（第 89-95 行），若有限制则更新。

- [ ] **Step 4: 构建验证**

  ```bash
  zsh -lc "cargo build -p openjax-gateway"
  ```
  Expected: 编译通过

- [ ] **Step 5: 运行 Gateway 全量测试**

  ```bash
  zsh -lc "cargo test -p openjax-gateway"
  ```
  Expected: 所有测试通过

- [ ] **Step 6: Commit**

  ```bash
  git add openjax-gateway/src/handlers/session.rs
  git commit -m "feat(gateway): session_action 增加 abort 分支"
  ```

---

## Task 4: TUI — Esc 中断任务

**Files:**
- Modify: `ui/tui/src/runtime.rs`

- [ ] **Step 1: 了解当前 dismiss_overlay 及调用处**

  阅读 `ui/tui/src/runtime.rs` 第 26-36 行（`dismiss_overlay` 函数）和第 152 行（调用处）。

- [ ] **Step 2: 修改 `dismiss_overlay` 签名，增加 abort turn 分支**

  将 `dismiss_overlay` 函数替换为：

  ```rust
  fn dismiss_overlay(
      app: &mut App,
      turn_task: &mut Option<tokio::task::JoinHandle<()>>,
      core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
  ) {
      if app.state.policy_picker.is_some() {
          app.dismiss_policy_picker();
      } else if app.is_slash_palette_active() {
          app.dismiss_slash_palette();
      } else if app.state.pending_approval.is_some() {
          app.defer_pending_approval();
      } else if turn_task.is_some() {
          abort_turn(app, turn_task, core_event_rx);
      } else {
          app.clear();
      }
  }

  fn abort_turn(
      app: &mut App,
      turn_task: &mut Option<tokio::task::JoinHandle<()>>,
      core_event_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>>,
  ) {
      if let Some(task) = turn_task.take() {
          task.abort();
      }
      // 丢弃残余事件
      core_event_rx.take();
      // 补发合成 TurnCompleted 以重置 status bar
      let turn_id = app.state.active_turn_id.unwrap_or(0);
      app.apply_core_event(openjax_protocol::Event::TurnCompleted { turn_id });
      app.set_live_status("已中断");
  }
  ```

- [ ] **Step 3: 更新调用处**

  将 `run()` 函数中（第 152 行）的：
  ```rust
  InputAction::DismissOverlay => dismiss_overlay(&mut app),
  ```
  改为：
  ```rust
  InputAction::DismissOverlay => dismiss_overlay(&mut app, &mut turn_task, &mut core_event_rx),
  ```

- [ ] **Step 4: 更新受影响的测试**

  `runtime.rs` 底部有一个测试直接调用 `dismiss_overlay`（测试 `dismiss_overlay_with_pending_approval_defers_request`），需要更新其调用签名：

  ```rust
  let mut turn_task: Option<tokio::task::JoinHandle<()>> = None;
  let mut core_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<openjax_protocol::Event>> = None;
  dismiss_overlay(&mut app, &mut turn_task, &mut core_event_rx);
  ```

- [ ] **Step 5: 增加新的 abort turn 测试**

  在 `runtime.rs` 的测试模块中添加：
  ```rust
  #[tokio::test]
  async fn abort_turn_clears_task_and_sets_status() {
      let mut app = App::default();
      // 模拟有 active turn
      app.apply_core_event(openjax_protocol::Event::TurnStarted { turn_id: 42 });
      // 创建一个永远不结束的任务
      let mut turn_task: Option<tokio::task::JoinHandle<()>> =
          Some(tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(9999)).await }));
      let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<openjax_protocol::Event>();
      drop(tx);
      let mut core_event_rx = Some(rx);

      abort_turn(&mut app, &mut turn_task, &mut core_event_rx);

      assert!(turn_task.is_none(), "turn_task 应已被清除");
      assert!(core_event_rx.is_none(), "core_event_rx 应已被清除");
      // active_turn_id 应通过合成 TurnCompleted 被清除
      assert!(app.state.active_turn_id.is_none());
      let status = app.state.live_messages.first().expect("应有 live status");
      assert!(status.content.contains("已中断"));
  }
  ```

- [ ] **Step 6: 运行 TUI 测试**

  ```bash
  zsh -lc "cargo test -p tui_next"
  ```
  Expected: 所有测试通过（包括新增的 `abort_turn_clears_task_and_sets_status`）

- [ ] **Step 7: Commit**

  ```bash
  git add ui/tui/src/runtime.rs
  git commit -m "feat(tui): Esc 键无 overlay 时中断当前 Agent 任务"
  ```

---

## Task 5: Web UI — 类型定义与图标

**Files:**
- Modify: `ui/web/src/types/gateway.ts`
- Modify: `ui/web/src/types/chat.ts`
- Modify: `ui/web/src/pic/icon/index.tsx`

- [ ] **Step 1: 在 `gateway.ts` 的 `StreamEvent.type` 联合类型中添加 `"turn_interrupted"`**

  在 `ui/web/src/types/gateway.ts` 的 `StreamEvent` 接口（第 205-234 行），`type` 字段的联合类型中，在 `"turn_completed"` 之后添加：
  ```ts
  | "turn_interrupted"
  ```

- [ ] **Step 2: 在 `chat.ts` 的 `ChatMessage` 接口中添加 `interrupted` 字段**

  在 `ui/web/src/types/chat.ts` 的 `ChatMessage` 接口（第 45-61 行），在 `reasoningBlocks` 字段之后添加：
  ```ts
  interrupted?: boolean;
  ```

- [ ] **Step 3: 在 `icon/index.tsx` 中添加 `StopCircleIcon`**

  在 `ui/web/src/pic/icon/index.tsx` 末尾添加（基于 `stop_circle.svg` 内容，将 `fill="#5C5C66"` 改为 `fill="currentColor"` 以跟随主题色）：
  ```tsx
  export function StopCircleIcon(props: IconProps) {
    return (
      <svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" {...props}>
        <path
          d="M512 42.666667C252.793333 42.666667 42.666667 252.793333 42.666667 512s210.126667 469.333333 469.333333 469.333333 469.333333-210.126667 469.333333-469.333333S771.206667 42.666667 512 42.666667z m213.333333 645.333333a37.373333 37.373333 0 0 1-37.333333 37.333333H336a37.373333 37.373333 0 0 1-37.333333-37.333333V336a37.373333 37.373333 0 0 1 37.333333-37.333333h352a37.373333 37.373333 0 0 1 37.333333 37.333333z"
          fill="currentColor"
        />
      </svg>
    );
  }
  ```

- [ ] **Step 4: 构建前端验证类型无误**

  ```bash
  zsh -lc "cd ui/web && pnpm build 2>&1 | tail -20"
  ```
  Expected: 构建成功，无类型错误

- [ ] **Step 5: Commit**

  ```bash
  git add ui/web/src/types/gateway.ts ui/web/src/types/chat.ts ui/web/src/pic/icon/index.tsx
  git commit -m "feat(web): 增加 turn_interrupted 类型、ChatMessage.interrupted 字段和 StopCircleIcon"
  ```

---

## Task 6: Web UI — gatewayClient + useChatApp

**Files:**
- Modify: `ui/web/src/lib/gatewayClient.ts`
- Modify: `ui/web/src/hooks/useChatApp.ts`

- [ ] **Step 1: 在 `gatewayClient.ts` 中找到现有 session action 方法作为参考**

  阅读 `gatewayClient.ts` 中 `clearConversation` 或 `compactConversation` 的实现，了解 session action 请求格式（`POST /api/v1/sessions/:id:clear`）。

- [ ] **Step 2: 在 `gatewayClient.ts` 中添加 `abortTurn` 方法**

  在 `GatewayClient` 类中，在 `clearConversation` 方法之后添加：
  ```ts
  async abortTurn(sessionId: string): Promise<void> {
    await this.request(`/api/v1/sessions/${sessionId}:abort`, {
      method: "POST"
    });
  }
  ```

- [ ] **Step 3: 在 `useChatApp.ts` 中计算 `isStreaming`**

  在 `activeSession` 的 `useMemo` 之后（第 212-215 行附近），添加：
  ```ts
  const isStreaming = useMemo(
    () =>
      activeSession != null &&
      activeSession.turnPhase === "streaming" &&
      activeSession.pendingApprovals.length === 0,
    [activeSession]
  );
  ```

- [ ] **Step 4: 在 `useChatApp.ts` 中添加 `abortTurn` 函数**

  仿照 `sendMessage` 的 `withAuthRetryRuntime` 模式，在 return 对象前添加（在现有 action 函数区域）：
  ```ts
  const abortTurn = useCallback(async () => {
    if (!state.activeSessionId) return;
    try {
      await withAuthRetryRuntime({ client, state, setState, clearAuthState, refreshPromiseRef }, () =>
        client.abortTurn(state.activeSessionId!)
      );
    } catch {
      // 静默忽略，abort 是 best-effort
    }
  }, [client, state.activeSessionId, clearAuthState]);
  ```

- [ ] **Step 5: 在 `useChatApp.ts` 的 return 对象中导出 `isStreaming` 和 `abortTurn`**

  在 return 对象（当前最后返回的那个大对象）中添加：
  ```ts
  isStreaming,
  abortTurn,
  ```

- [ ] **Step 6: 运行前端测试**

  ```bash
  zsh -lc "cd ui/web && pnpm test -- src/lib/gatewayClient.test.ts"
  ```
  Expected: 现有测试通过，无回归

- [ ] **Step 7: Commit**

  ```bash
  git add ui/web/src/lib/gatewayClient.ts ui/web/src/hooks/useChatApp.ts
  git commit -m "feat(web): 添加 abortTurn 方法和 isStreaming 计算值"
  ```

---

## Task 7: Web UI — Composer Stop 按钮 + App.tsx

**Files:**
- Modify: `ui/web/src/components/composer/ComposerInput.tsx`
- Modify: `ui/web/src/components/composer/index.tsx`
- Modify: `ui/web/src/App.tsx`

- [ ] **Step 1: 在 `ComposerInput.tsx` 中导入 `StopCircleIcon` 并更新 props**

  在 `ComposerInput.tsx` 中：

  1. 在 import 行将 `SendIcon` 改为同时导入 `StopCircleIcon`：
     ```ts
     import { SendIcon, StopCircleIcon } from "../../pic/icon";
     ```

  2. 在 `ComposerInputProps` 接口中添加：
     ```ts
     isStreaming?: boolean;
     onStop?: () => void;
     ```

  3. 在函数参数中添加 `isStreaming` 和 `onStop`

  4. 将发送按钮区域替换为条件渲染：
     ```tsx
     {isStreaming ? (
       <button
         type="button"
         className="composer-stop-btn"
         onClick={onStop}
         aria-label="停止"
         title="停止"
       >
         <StopCircleIcon aria-hidden="true" />
       </button>
     ) : (
       <button
         type="button"
         className={`composer-send-btn ${hasContent && !disabled ? "ready" : ""}`}
         onClick={onSubmit}
         disabled={disabled || !hasContent}
         aria-label="发送"
         title="发送"
       >
         <SendIcon aria-hidden="true" />
       </button>
     )}
     ```

- [ ] **Step 2: 在 `composer/index.tsx` 中透传 `isStreaming` 和 `onStop`**

  1. 在 `ComposerProps` 接口中添加：
     ```ts
     isStreaming?: boolean;
     onStop?: () => void;
     ```

  2. 在函数参数中添加 `isStreaming` 和 `onStop`

  3. 在 `<ComposerInput ... />` 中添加：
     ```tsx
     isStreaming={isStreaming}
     onStop={onStop}
     ```

- [ ] **Step 3: 在 `App.tsx` 中从 `useChatApp` 取出并传入**

  1. 在 `useChatApp()` 解构处添加 `isStreaming` 和 `abortTurn`

  2. 在 `<Composer ... />` 处添加：
     ```tsx
     isStreaming={isStreaming}
     onStop={() => void abortTurn()}
     ```

- [ ] **Step 4: 构建验证**

  ```bash
  zsh -lc "cd ui/web && pnpm build 2>&1 | tail -20"
  ```
  Expected: 构建成功

- [ ] **Step 5: 运行 Composer 相关测试**

  ```bash
  zsh -lc "cd ui/web && pnpm test -- src/components/composer/index.test.tsx"
  ```
  Expected: 通过

- [ ] **Step 6: Commit**

  ```bash
  git add ui/web/src/components/composer/ComposerInput.tsx ui/web/src/components/composer/index.tsx ui/web/src/App.tsx
  git commit -m "feat(web): Composer 流式时显示 Stop 按钮"
  ```

---

## Task 8: Web UI — reducer 处理 turn_interrupted + MessageList 显示标记

**Files:**
- Modify: `ui/web/src/lib/session-events/reducer.ts`
- Modify: `ui/web/src/components/MessageList.tsx`

- [ ] **Step 1: 写失败测试（reducer.test.ts）**

  在 `ui/web/src/lib/session-events/reducer.test.ts` 中新增测试：
  ```ts
  describe("turn_interrupted", () => {
    it("marks the streaming assistant message as interrupted and sets turnPhase to completed", () => {
      // 构造一个有流式消息的 session
      let session = makeEmptySession();
      // 模拟 response_started + response_text_delta 场景
      session = applySessionEvent(session, makeEvent("response_started", { turn_id: "t1" }));
      session = applySessionEvent(session, makeEvent("response_text_delta", {
        turn_id: "t1",
        content_delta: "hello"
      }));
      // 应用 turn_interrupted
      const result = applySessionEvent(session, makeEvent("turn_interrupted", {
        turn_id: "t1",
        reason: "user_abort"
      }));
      expect(result.turnPhase).toBe("completed");
      const msg = result.messages.find(m => m.turnId === "t1" && m.role === "assistant");
      expect(msg?.interrupted).toBe(true);
    });
  });
  ```

  参考同文件中现有的 `makeEvent` / `makeEmptySession` 辅助函数写法（或直接参照已有测试用例模式）。

- [ ] **Step 2: 运行测试验证失败**

  ```bash
  zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/reducer.test.ts 2>&1 | tail -20"
  ```
  Expected: FAIL — `turn_interrupted` 未处理

- [ ] **Step 3: 在 `reducer.ts` 的 `applySingleSessionEvent` 中处理 `turn_interrupted`**

  在 `applySingleSessionEvent` 函数中，在 `turn_completed` 的处理分支附近，找到 `switch(event.type)` 或条件链，添加：

  ```ts
  if (event.type === "turn_interrupted") {
    // 收尾逻辑与 response_completed 类似：将 draft assistant 消息定稿
    const turnId = event.turn_id;
    const updatedMessages = next.messages.map((msg) => {
      if (msg.turnId === turnId && msg.role === "assistant" && msg.isDraft) {
        return { ...msg, isDraft: false, interrupted: true };
      }
      return msg;
    });
    return {
      ...next,
      messages: updatedMessages,
      turnPhase: "completed" as const,
    };
  }
  ```

- [ ] **Step 4: 运行测试验证通过**

  ```bash
  zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/reducer.test.ts 2>&1 | tail -20"
  ```
  Expected: PASS

- [ ] **Step 5: 在 `MessageList.tsx` 中显示已中断标记**

  阅读 `MessageList.tsx` 中 assistant 文本消息的渲染逻辑，找到消息内容渲染处（通常是 `message.content` 或 markdown 渲染）。

  在 assistant 消息气泡末尾，条件渲染中断标记：
  ```tsx
  {message.interrupted && (
    <span className="message-interrupted-badge" aria-label="已中断">
      [已中断]
    </span>
  )}
  ```

  在 `src/styles/messages.css`（或对应样式文件）中添加样式：
  ```css
  .message-interrupted-badge {
    display: inline-block;
    margin-left: 0.4em;
    font-size: 0.75em;
    color: var(--color-text-muted, #999);
    opacity: 0.7;
  }
  ```

- [ ] **Step 6: 运行全量前端测试**

  ```bash
  zsh -lc "cd ui/web && pnpm test"
  ```
  Expected: 所有测试通过

- [ ] **Step 7: Commit**

  ```bash
  git add ui/web/src/lib/session-events/reducer.ts ui/web/src/components/MessageList.tsx ui/web/src/styles/messages.css
  git commit -m "feat(web): reducer 处理 turn_interrupted，MessageList 显示已中断标记"
  ```

---

## 最终验证

- [ ] **Step 1: 全量 Rust 构建**

  ```bash
  zsh -lc "cargo build --workspace"
  ```
  Expected: 构建成功

- [ ] **Step 2: 全量 Rust 测试**

  ```bash
  zsh -lc "cargo test -p openjax-gateway && cargo test -p tui_next"
  ```
  Expected: 全部通过

- [ ] **Step 3: 全量前端测试**

  ```bash
  zsh -lc "cd ui/web && pnpm test"
  ```
  Expected: 全部通过

- [ ] **Step 4: 手动端到端验证**

  启动开发环境（`make run-web-dev`），测试以下场景：
  - 发送一个需要多步工具调用的任务，任务进行中点击 Stop 按钮
  - 确认 Stop 按钮变回 Send 按钮
  - 确认已输出的部分内容保留，末尾显示"[已中断]"
  - TUI 中发送任务，运行中按 Esc，确认任务中止，输入框恢复
  - 审批面板打开时按 Esc，确认先关闭审批面板（不触发 abort）
