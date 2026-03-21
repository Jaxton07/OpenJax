# OpenJax 跨语言协议 v1（草案）

状态：`Draft`  
版本：`v1`  
适用范围：`openjaxd`（Rust）与 Python SDK/TUI/Bot 之间的通信。

---

## 1. 设计目标

1. Rust 保持内核能力与安全边界。
2. Python 通过统一协议访问会话、turn、事件流与审批。
3. 协议字段可追踪、可扩展、可向前兼容。

---

## 2. 传输与分帧（v1 固定）

v1 固定使用：`stdio + JSONL`（每行一个完整 JSON 对象）。

约束：
1. 一条消息必须是单行完整 JSON。
2. 每条消息必须包含 `protocol_version: "v1"`。
3. 消息类型通过 `kind` 区分：`request` / `response` / `event`。

---

## 3. 信封模型

### 3.1 Request

```json
{
  "protocol_version": "v1",
  "kind": "request",
  "request_id": "req_001",
  "session_id": "sess_001",
  "method": "submit_turn",
  "params": {}
}
```

说明：
1. `request_id`：客户端生成，单进程内唯一。
2. `session_id`：`start_session` 之外的方法必填。
3. `params`：各方法参数对象。

### 3.2 Response

```json
{
  "protocol_version": "v1",
  "kind": "response",
  "request_id": "req_001",
  "ok": true,
  "result": {}
}
```

或错误响应：

```json
{
  "protocol_version": "v1",
  "kind": "response",
  "request_id": "req_001",
  "ok": false,
  "error": {
    "code": "INVALID_PARAMS",
    "message": "field `input` is required",
    "retriable": false,
    "details": {}
  }
}
```

### 3.3 Event

```json
{
  "protocol_version": "v1",
  "kind": "event",
  "session_id": "sess_001",
  "turn_id": "turn_001",
  "event_type": "response_text_delta",
  "payload": {}
}
```

---

## 4. 操作集（v1）

### 4.1 `start_session`

用途：创建会话并返回 `session_id`。  
请求参数：
1. `client_name`（可选）
2. `metadata`（可选对象）

响应结果：
1. `session_id`
2. `created_at`

### 4.2 `submit_turn`

用途：向会话提交用户输入并创建新 turn。  
请求参数：
1. `input`（必填）
2. `metadata`（可选对象）

响应结果：
1. `turn_id`
2. `accepted`（布尔）

### 4.3 `stream_events`

用途：订阅会话事件流。  
请求参数：
1. `from_seq`（可选，默认最新位置）

响应结果：
1. `subscribed`（布尔）

说明：事件将以 `kind=event` 持续输出，直到会话关闭或客户端取消订阅。

### 4.4 `resolve_approval`

用途：回传审批决策。  
请求参数：
1. `turn_id`（必填）
2. `request_id`（必填，来自 `approval_requested`）
3. `approved`（必填）
4. `reason`（可选）

响应结果：
1. `resolved`（布尔）

### 4.5 `shutdown_session`

用途：关闭会话并回收资源。  
请求参数：无。  
响应结果：
1. `closed`（布尔）

---

## 5. 事件模型（v1）

以下事件类型为 v1 最小集合：

1. `turn_started`
2. `tool_call_started`
3. `tool_args_delta`
4. `tool_call_progress`
5. `tool_call_completed`
6. `tool_call_failed`
7. `response_started`
8. `response_text_delta`
9. `response_completed`
10. `assistant_message`（deprecated compatibility only）
11. `approval_requested`
12. `approval_resolved`
13. `turn_completed`
14. `session_shutdown_complete`
15. `error`

说明：
1. `assistant_delta` 已移除，客户端应仅使用 `response_*` 事件作为流式正文来源。
2. `assistant_message` 已标记为 `deprecated`，仅作为兼容旧消费者的事件保留。
3. A 阶段：保留兼容桥接，允许旧实现继续读写 `assistant_message`。
4. B 阶段：新实现默认只以 `response_*` 作为正文主链路，`assistant_message` 仅在 legacy fallback 中出现。
5. C 阶段：移除 `assistant_message` 的推荐生产路径，仅保留文档与旧数据兼容说明。

事件字段原则：
1. `event_type` 固定字符串枚举。
2. `payload` 字段按事件类型扩展。
3. 所有事件必须携带 `session_id`，turn 相关事件必须携带 `turn_id`。

> **注意**：所有工具调用事件（`tool_call_started`、`tool_call_completed`、`tool_args_delta`、`tool_call_progress`、`tool_call_ready`、`tool_call_failed`）均包含一个可选的 `display_name` 字段（`Option<String>`），用于 UI 显示的友好名称。JSON 序列化时可省略。

与 `openjax-protocol::Event` 的建议映射：

| Rust Event 变体 | v1 `event_type` |
|---|---|
| `TurnStarted` | `turn_started` |
| `ToolCallStarted` | `tool_call_started` |
| `ToolCallArgsDelta` | `tool_args_delta` |
| `ToolCallProgress` | `tool_call_progress` |
| `ToolCallCompleted` | `tool_call_completed` |
| `ToolCallFailed` | `tool_call_failed` |
| `ResponseStarted` | `response_started` |
| `ResponseTextDelta` | `response_text_delta` |
| `ResponseCompleted` | `response_completed` |
| `AssistantMessage` | `assistant_message`（deprecated compatibility only） |
| `ApprovalRequested` | `approval_requested` |
| `ApprovalResolved` | `approval_resolved` |
| `TurnCompleted` | `turn_completed` |
| `ShutdownComplete` | `session_shutdown_complete` |

---

## 6. 错误模型（统一）

错误对象：

```json
{
  "code": "TIMEOUT",
  "message": "approval timed out",
  "retriable": true,
  "details": {
    "request_id": "appr_001"
  }
}
```

标准错误码（v1）：
1. `INVALID_REQUEST`
2. `INVALID_PARAMS`
3. `SESSION_NOT_FOUND`
4. `TURN_NOT_FOUND`
5. `APPROVAL_NOT_FOUND`
6. `TIMEOUT`
7. `INTERNAL_ERROR`
8. `NOT_IMPLEMENTED`

---

## 7. ID 与追踪规则

1. `request_id`：请求级唯一。
2. `session_id`：会话级唯一。
3. `turn_id`：会话内唯一（建议全局也唯一，便于日志关联）。
4. 审批请求 ID 使用 `approval_request_id`（当前字段沿用 `request_id`，在 `approval_requested` payload 中出现）。

日志建议统一输出：
1. `request_id`
2. `session_id`
3. `turn_id`
4. `event_type`

---

## 8. 审批闭环语义（v1）

1. 内核发出 `approval_requested` 后进入等待态。
2. 客户端必须回传 `resolve_approval`。
3. 若超时未回传，触发 `error(code=TIMEOUT)` 并默认拒绝（保守策略）。
4. 客户端可通过 `resolve_approval(approved=false, reason="cancelled_by_user")` 主动取消。

推荐默认超时：`approval_timeout_ms = 60000`（实现可配置）。

---

## 9. 时序（单 turn）

1. `start_session` -> 返回 `session_id`
2. `stream_events` -> 建立事件流
3. `submit_turn` -> 返回 `turn_id`
4. daemon 持续发事件：
   - `turn_started`
   - `tool_call_started` / `tool_call_completed`（可多次）
   - `approval_requested` -> 客户端 `resolve_approval` -> `approval_resolved`（可选）
   - `response_started`
   - `response_text_delta`（可多次）
   - `response_completed`
   - `assistant_message`（deprecated compatibility only，非权威完成态）
   - `turn_completed`
5. `shutdown_session` -> `session_shutdown_complete`

---

## 10. 向前兼容规则

1. 新增字段：允许。
2. 新增事件类型：允许，旧客户端按未知事件忽略或降级展示。
3. 删除字段/语义变更：必须升级 major。
4. schema 与文档必须同 PR 更新。
