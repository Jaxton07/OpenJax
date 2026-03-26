# 设计文档：Agent 任务中断功能

**日期：** 2026-03-26
**状态：** 已批准
**影响范围：** TUI、openjax-gateway、ui/web

---

## 1. 背景与目标

当前 Agent 执行任务时没有任何取消机制。对于执行时间较长、或出现偏差/卡死的任务，用户无法干预，只能等待超时或重启进程。

**目标：** 在 TUI 和 Web UI 中提供"中断当前任务"的交互，立即停止运行中的 Agent 轮次，保留已输出的部分内容并标记为"已中断"状态。

**不在范围内：** 修改 `openjax-core` 的 Agent 结构；支持暂停后恢复；子 Agent 的级联中断。

---

## 2. 核心机制

**方案：`JoinHandle::abort()`**

- 不修改 `openjax-core`，通过 tokio 任务层强制终止
- abort 触发在任意 `.await` 点（合作式取消），正在执行的系统调用不会被截断
- abort 前已经发出的 delta 事件（文字、工具调用）已到达 UI 层和持久化层，内容不丢失
- abort 后由外部补发一个合成的 `turn_interrupted` 事件，恢复 UI 状态

---

## 3. 各层设计

### 3.1 TUI

**触发方式：** Esc 键（overlay 优先策略）

Esc 的处理优先级保持不变：
1. Policy Picker 打开 → 关闭 Policy Picker
2. Slash 面板打开 → 关闭 Slash 面板
3. 审批等待中 → 推迟审批（defer）
4. **turn_task 运行中 → 中断任务（新增）**
5. 否则 → 清空输入框

**改动文件：**

`ui/tui/src/runtime.rs`

- `dismiss_overlay` 函数签名扩展，增加 `turn_task: &mut Option<JoinHandle<()>>` 和 `core_event_rx: &mut Option<UnboundedReceiver<Event>>` 参数
- 在第 4 个分支中调用新辅助函数 `abort_turn`

新增 `abort_turn` 函数逻辑：
1. `turn_task.take().unwrap().abort()`
2. `core_event_rx.take()` — 丢弃残余事件
3. `app.set_live_status("已中断")` — 向用户反馈
4. 向 app 补发合成的 `TurnCompleted` 事件，使 status bar 正确清空、输入框解锁

**状态恢复：**
- `turn_task = None` → 输入框解锁
- `core_event_rx = None` → drain 循环退出
- `active_turn_id = None` → status bar 清空（由合成 TurnCompleted 触发）
- live status "已中断" 通过现有超时机制自动消退

---

### 3.2 Gateway

**改动文件：**

**`src/state/runtime.rs`**
`SessionRuntime` 增加字段：
```rust
pub current_turn_abort_handle: Option<tokio::task::AbortHandle>,
```

**`src/state/events.rs`**
`run_turn_task` 开始时：
- 取得 `submit_task.abort_handle()`，存入 `session_runtime.current_turn_abort_handle`

任务结束时：
- 清除 `current_turn_abort_handle`

`submit_task.await` 返回 `Err(JoinError)` 时（即被 abort）：
- 通过 `publish_and_persist_event` 广播合成事件：
  - `event_type = "turn_interrupted"`
  - `payload = { "turn_id": "<public_turn_id>", "reason": "user_abort" }`
- 将 `TurnStatus` 标记为 `Failed`（复用现有失败状态，后续可扩展 `Interrupted` 变体）

**`src/handlers/session.rs`**
新增 `abort_session_turn` handler：
1. 获取 session
2. `session.current_turn_abort_handle.take()` — 幂等，取到 None 直接返回 200
3. 调用 `.abort()`
4. 返回 200（不等待任务实际完成）

**`src/lib.rs`**
注册路由：
```
POST /api/v1/sessions/:session_id:abort
```
该路由加入受保护路由组（需 Bearer token）。

---

### 3.3 Web UI

**触发方式：** Stop 按钮（任务运行中替换 Send 按钮）

**改动文件：**

**`src/pic/icon/index.tsx`**
添加 `StopCircleIcon`：
```tsx
import stopCircleSvg from "./stop_circle.svg?react";
export const StopCircleIcon = stopCircleSvg;
```

**`src/lib/gatewayClient.ts`**
新增方法：
```ts
abortTurn(sessionId: string): Promise<void>
// POST /api/v1/sessions/:sessionId:abort
```

**`src/hooks/useChatApp.ts`**
- 新增 `abortTurn` action，调用 `gatewayClient.abortTurn(activeSessionId)`
- 向外暴露 `isStreaming: boolean`（当前 active session 有 turn 处于 running 状态且非审批等待中）

**`src/components/composer/index.tsx`**
新增 props：
```ts
isStreaming?: boolean;
onStop?: () => void;
```
透传给 `ComposerInput`。

**`src/components/composer/ComposerInput.tsx`**
发送区渲染逻辑：
```tsx
{isStreaming ? (
  <button type="button" onClick={onStop} aria-label="停止" className="composer-stop-btn">
    <StopCircleIcon aria-hidden="true" />
  </button>
) : (
  <button
    type="button"
    className={`composer-send-btn ${hasContent && !disabled ? "ready" : ""}`}
    onClick={onSubmit}
    disabled={disabled || !hasContent}
    aria-label="发送"
  >
    <SendIcon aria-hidden="true" />
  </button>
)}
```

Stop 按钮不受 `disabled` / `hasContent` 限制，始终可点击。
审批等待期间 `isStreaming` 返回 false，不显示 Stop 按钮。

**`src/types/gateway.ts`**
新增事件类型：
```ts
| { type: "turn_interrupted"; payload: { turn_id: string; reason: string } }
```

**`src/lib/session-events/reducer.ts`**
处理 `turn_interrupted` 事件：
- 关闭当前流式消息（等同于 `response_completed` 的收尾逻辑）
- 在消息上标记 `interrupted: true`（需扩展 `ChatMessage` 类型）
- 清除 session 的 streaming 状态

**`src/types/chat.ts`**
`ChatMessage` 类型新增可选字段：
```ts
interrupted?: boolean;
```

**`src/components/MessageList.tsx`**（或对应消息气泡组件）
当 `message.interrupted === true` 时，在消息末尾附加一个小标记，例如"[已中断]"或一个图标徽标。

---

## 4. 错误处理与边界情况

| 场景 | 处理 |
|------|------|
| 双击 Stop / 重复调用 abort | `take()` 取到 None 直接返回，幂等 |
| abort 时任务已自然完成 | tokio abort 对已完成任务是 no-op；Gateway 取到 None 返回 200；TUI 侧 turn_task 已被 drain 消费 |
| abort 时审批面板打开 | TUI: Esc overlay 优先，不会触发 abort；Web: isStreaming 为 false，不显示 Stop |
| 网络断开时点击 Stop | Gateway handler 执行 abort 后写入持久化；前端重连后通过 timeline hydration 恢复，消息显示"已中断" |
| `turn_interrupted` 未收到（SSE 断线）| timeline hydration 拉取持久化事件，reducer 补处理 `turn_interrupted` |

---

## 5. 数据流总结

```
用户触发中断
    │
    ├─ TUI: turn_task.abort() + 补发合成 TurnCompleted
    │       → 输入框解锁，status bar 清空，live status "已中断"
    │
    └─ Web: POST /sessions/:id:abort
            → Gateway abort submit_task
            → 发布 turn_interrupted 事件（持久化 + SSE 广播）
            → 前端 reducer 关闭流式，标记消息 interrupted=true
            → Stop 按钮变回 Send 按钮
```

---

## 6. 改动文件汇总

| 文件 | 类型 | 行数估算 |
|------|------|----------|
| `ui/tui/src/runtime.rs` | 修改 | ~30 |
| `openjax-gateway/src/state/runtime.rs` | 修改 | ~5 |
| `openjax-gateway/src/state/events.rs` | 修改 | ~25 |
| `openjax-gateway/src/handlers/session.rs` | 修改 | ~20 |
| `openjax-gateway/src/lib.rs` | 修改 | ~3 |
| `ui/web/src/pic/icon/index.tsx` | 修改 | ~3 |
| `ui/web/src/lib/gatewayClient.ts` | 修改 | ~8 |
| `ui/web/src/hooks/useChatApp.ts` | 修改 | ~15 |
| `ui/web/src/components/composer/index.tsx` | 修改 | ~8 |
| `ui/web/src/components/composer/ComposerInput.tsx` | 修改 | ~15 |
| `ui/web/src/types/gateway.ts` | 修改 | ~5 |
| `ui/web/src/lib/session-events/reducer.ts` | 修改 | ~15 |
| `ui/web/src/types/chat.ts` | 修改 | ~3 |
| `ui/web/src/components/MessageList.tsx` | 修改 | ~5 |
| **合计** | | **~160 行** |

---

## 7. 不在此次范围内

- 子 Agent 的级联中断
- 暂停后恢复执行
- 中断后自动重试
- `TurnStatus::Interrupted` 独立枚举变体（当前复用 `Failed`，后续可扩展）
