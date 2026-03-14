# streaming 模块

`openjax-core/src/streaming/` 是后端统一流式子系统，负责事件生命周期编排、provider SSE 归一化、分发背压与回放窗口能力。

## 目录与职责

- `event.rs`: 流式领域事件模型（turn/response/tool/approval/usage/error）。
- `orchestrator.rs`: 响应流状态机（`started -> delta* -> completed/error`）。
- `parser/`: provider SSE 解析抽象与实现（`openai`、`anthropic`）。
- `sink.rs`: 有界分发通道与背压策略（`DropNewest` / `RejectProducer`）。
- `replay.rs`: 会话级回放窗口与越窗错误语义。

## 后端接入要点

1. provider 只做 provider-specific 字段提取，不再手写 SSE 行切分。
2. 读取流 chunk 后统一走：
   - `parser.push_chunk(bytes)` 处理完整帧
   - `parser.finish()` 处理尾包
3. 业务层仅消费规范化事件并发到 `submit_with_sink`。

## WebUI 流式接入指引（SSE）

### 1) 订阅网关 SSE

- 端点：`GET /api/v1/sessions/:session_id/events`
- 恢复参数：
  - query: `after_event_seq`
  - header: `Last-Event-ID`
- 线格式：`Envelope + payload`，关键字段：
  - `event_seq`
  - `turn_seq`
  - `type`
  - `payload`

### 2) 前端最小事件渲染状态机

1. `response_started`: 初始化当前 turn 的流式缓冲。
2. `response_text_delta`: 追加 `payload.content_delta` 并实时渲染。
3. `response_completed`: 使用 `payload.content` 收敛最终文案。
4. `response_error`: 显示可重试错误（读取 `code/message/retryable`）。
5. `tool_call_started/args_delta/progress/completed/failed`: 同一时间线展示工具执行过程。
6. `approval_requested/resolved`: 与工具事件并流展示审批状态。

### 3) 推荐前端数据结构

- `sessionStreamState`
  - `lastEventSeq: number`
  - `turns: Record<string, TurnStreamState>`
- `TurnStreamState`
  - `responseBuffer: string`
  - `toolCalls: Record<string, ToolCallState>`
  - `approvals: Record<string, ApprovalState>`

### 4) 断线重连策略

1. 每收到一条 SSE 就持久化 `lastEventSeq`。
2. 重连优先带 `after_event_seq=lastEventSeq`。
3. 若收到 `REPLAY_WINDOW_EXCEEDED`，提示用户刷新会话并重新拉流。

## 验证建议

- 单测：`parser` 分段拼接、`sink` 背压、`replay` 越窗、`orchestrator` 顺序。
- 集成：`cargo test -p openjax-core --test m6_submit_stream`。
