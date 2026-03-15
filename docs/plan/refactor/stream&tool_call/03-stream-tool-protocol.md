# 03. Stream + Tool Call 协议设计与固化

## 1. 设计原则

1. 事件单一语义，不重复表达同一事实。
2. 文本流与控制流解耦。
3. 每个事件可独立回放，依赖 `event_seq` 单调递增。
4. 兼容现有客户端，新增字段尽量向后兼容。

## 2. 事件信封（保持不变）

```json
{
  "event_seq": 123,
  "turn_seq": 45,
  "type": "response_text_delta",
  "payload": {}
}
```

## 3. 数据面事件（高频）

1. `response_started`
   - payload:
     - `stream_source`: `model_live|synthetic|replay`
2. `response_text_delta`
   - payload:
     - `content_delta`: string
     - `stream_source`
3. `response_completed`
   - payload:
     - `content`: string
     - `stream_source`
4. `response_error`
   - payload:
     - `code`
     - `message`
     - `retryable`

约束：

1. 文本流中不混入 tool 参数原始片段。
2. `response_completed.content` 必须与 delta 收敛后内容一致。

## 4. 控制面事件（低频）

1. `tool_call_started`
2. `tool_args_delta`
3. `tool_call_ready`
4. `tool_call_progress`
5. `tool_call_completed`
6. `tool_call_failed`
7. `approval_requested`
8. `approval_resolved`
9. `turn_started`
10. `turn_completed`

说明：

1. `approval_*` 由工具执行层产生，不由 dispatcher 产生。
2. `tool_call_ready` 表示参数完整可执行，便于前端显示“已就绪”状态。

## 5. 分发器判定协议

新增内部判定事件（仅服务端内部，不对外 SSE）：

1. `dispatch_probe_started`
2. `dispatch_branch_locked`
3. `dispatch_probe_timeout`
4. `dispatch_probe_error`

字段建议：

1. `turn_id`
2. `locked_branch`: `text|tool_call`
3. `probe_ms`
4. `signal_source`: `provider_structured|adapter_hint|heuristic`

## 6. 兼容性策略

1. 旧客户端兼容
   - 保持既有核心事件类型可用。
   - 新增事件类型仅增不改。
2. `assistant_message` 迁移
   - 推荐逐步退化为兼容事件，不再作为主渲染依据。
   - 主渲染依据固定为 `response_*` 事件。
3. 回放兼容
   - 仍使用 `after_event_seq`。
   - 回放窗口不足时返回 `REPLAY_WINDOW_EXCEEDED`。

## 7. 误发防护规则（强约束）

1. `PROBING` 未锁定前，不向前端发 `response_text_delta`。
2. 一旦锁定 `tool_call`，丢弃 probing 的文本暂存，不得补发。
3. 一旦锁定 `text`，禁止后续 tool 事件进入同一 turn 的文本通道。
4. 若 provider 同 turn 同时给出 text 和 tool_call：
   - 以结构化 tool_call 优先，进入工具分支并记录冲突日志。

