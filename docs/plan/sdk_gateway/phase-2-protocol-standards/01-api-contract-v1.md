# 01 API Contract v1

## 协议基线

- 传输：HTTP JSON
- 输出模式：SSE（流式）+ Polling（非流式）
- 版本前缀：`/api/v1`
- 鉴权：`Authorization: Bearer <api_key>`

## 同步接口

### `POST /api/v1/sessions`

- 作用：创建会话
- 请求体：

```json
{}
```

- 响应体：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "timestamp": "2026-03-08T12:00:00Z"
}
```

### `POST /api/v1/sessions/{session_id}/turns`

- 作用：提交用户输入并触发回合
- 请求体：

```json
{
  "input": "tool:list_dir dir_path=.",
  "metadata": {
    "client_turn_id": "c_turn_001"
  }
}
```

- 响应体：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "turn_id": "turn_xxx",
  "timestamp": "2026-03-08T12:00:01Z"
}
```

### `GET /api/v1/sessions/{session_id}/turns/{turn_id}`

- 作用：非流式模式下查询回合状态与最终结果（Polling）。
- 响应体（进行中）：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "turn_id": "turn_xxx",
  "status": "running",
  "timestamp": "2026-03-08T12:00:01Z"
}
```

- 响应体（完成）：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "turn_id": "turn_xxx",
  "status": "completed",
  "assistant_message": "final answer text",
  "timestamp": "2026-03-08T12:00:08Z"
}
```

### `POST /api/v1/sessions/{session_id}/approvals/{approval_id}:resolve`

- 作用：回传审批决策
- 请求体：

```json
{
  "approved": true,
  "reason": "approved by owner"
}
```

- 响应体：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "approval_id": "approval_xxx",
  "status": "resolved",
  "timestamp": "2026-03-08T12:00:02Z"
}
```

### `POST /api/v1/sessions/{session_id}:clear`

- 作用：清空当前会话历史并开始新的对话上下文（保留 `session_id`）。
- 请求体：

```json
{
  "reason": "user requested clear"
}
```

- 响应体：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "status": "cleared",
  "timestamp": "2026-03-08T12:00:02Z"
}
```

### `POST /api/v1/sessions/{session_id}:compact`

- 作用：压缩当前会话上下文，保留关键上下文并降低历史 token 成本。
- 请求体：

```json
{
  "strategy": "default"
}
```

- 响应体（v1 目标）：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "status": "compacted",
  "timestamp": "2026-03-08T12:00:02Z"
}
```

- 当前约束：`compact` 在 core 尚未实现时返回 `NOT_IMPLEMENTED`（见 `03-error-model-and-codes.md`）。

### `DELETE /api/v1/sessions/{session_id}`

- 作用：关闭会话
- 响应体：

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "status": "shutdown",
  "timestamp": "2026-03-08T12:00:03Z"
}
```

## 字段约束（统一）

- `request_id`: 每次请求唯一标识。
- `session_id`: 会话唯一标识。
- `turn_id`: 回合唯一标识（仅 turn 相关接口/事件存在）。
- `event_seq`: 同一会话下单调递增（仅事件流存在）。
- `timestamp`: RFC3339 UTC 时间。

## 聊天命令桥接（第三方 IM 适配）

- 网关可识别聊天命令并映射到显式接口，避免将命令语义下沉到 core。
- 默认映射：
  - `/clear` -> `POST /api/v1/sessions/{session_id}:clear`
  - `/compact` -> `POST /api/v1/sessions/{session_id}:compact`
- 适用场景：Telegram 等单会话输入框，不提供“新建会话”按钮。

## 双模式接入约定（v1）

- 流式客户端：`submit_turn` + SSE `stream_events`。
- 非流式客户端：`submit_turn` + `GET /turns/{turn_id}` 轮询直到 `status=completed|failed`。
- 两种模式共享同一 `turn_id`、错误结构、鉴权与审计规则。
