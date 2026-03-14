# 02 Event Contract And API

状态：`done`

## 事件契约目标

1. 全面使用生命周期事件：
- `turn_started`
- `response_started`
- `response_text_delta`
- `response_completed`
- `response_error`
- `turn_completed`

2. 工具调用流式事件：
- `tool_call_started`
- `tool_args_delta`
- `tool_call_progress`
- `tool_call_completed`
- `tool_call_failed`

3. 审批并流事件：
- `approval_requested`
- `approval_resolved`

## API 重定义原则（破坏式）

1. 不保留旧事件兼容桥接。
2. SSE 事件体以语义完整为主，前端按新契约重写。
3. `Last-Event-ID` 与 `after_event_seq` 保持恢复语义，但事件 payload 可重构。

## 错误模型

1. 流错误必须可区分：上游失败、回放越窗、消费者滞后。
2. 错误 payload 必须包含 `code/message/retryable`。

## 已完成结果

1. `AssistantDelta` 已从协议与消费链路移除。
2. `event.schema.json` 已更新为新事件集合（含 tool 增量事件）。
3. 协议文档与示例已切到新事件。
