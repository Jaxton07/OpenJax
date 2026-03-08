# 02 SSE Event Contract v1

> 本文仅定义流式模式（SSE）契约。非流式模式见 `01-api-contract-v1.md` 的 Polling 接口。

## 流式通道

### `GET /api/v1/sessions/{session_id}/events`

- `Content-Type: text/event-stream`
- 事件以 `event:` + `data:` 输出，`data` 为 JSON。

## 统一事件包

```json
{
  "request_id": "req_xxx",
  "session_id": "sess_xxx",
  "turn_id": "turn_xxx",
  "event_seq": 12,
  "timestamp": "2026-03-08T12:00:05Z",
  "type": "assistant_delta",
  "payload": {}
}
```

## 事件类型（v1）

- `turn_started`
- `assistant_delta`
- `assistant_message`
- `tool_call_started`
- `tool_call_completed`
- `approval_requested`
- `approval_resolved`
- `turn_completed`
- `session_shutdown`
- `error`

## 顺序约束

- 同一 `turn_id` 内必须先出现 `turn_started`。
- `turn_completed` 是回合结束标志，之后不再出现该回合事件。
- `assistant_delta` 可重复出现，`assistant_message` 为该回合最终文本。
- `event_seq` 在同一 `session_id` 内严格递增。

## 重连约束

- 客户端可基于最近 `event_seq` 断点续读。
- 网关应支持 `Last-Event-ID` 或等价查询参数（实现细节在 phase-3 定义）。
