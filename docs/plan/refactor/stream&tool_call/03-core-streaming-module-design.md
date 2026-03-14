# 03 Core Streaming Module Design

状态：`in_progress`

## 设计目标

- 把流式职责从 `planner` 中剥离，形成可复用组件。
- 统一响应流事件发射与终态收敛逻辑。

## 已落地接口

1. `ResponseStreamOrchestrator`
- `on_delta(delta) -> Vec<Event>`
- `emit_completed(content) -> (resolved, Event)`
- `emit_error(code, message, retryable) -> Event`

2. `ReplayBuffer<T>`
- `push(item) -> seq`
- `replay_from(after_seq)`
- 越窗错误返回 `ReplayWindowError`。

3. `StreamDispatcher<T>`
- bounded channel
- 背压策略：`DropNewest` / `RejectProducer`

4. `parser` 抽象
- `SseParser` trait
- `openai` / `anthropic` 基础实现

## 下一步实现

1. 将 chat-completions 与 anthropic 的 SSE 解析逐步切到 `SseParser` trait 实现（当前已先收敛 `parse_sse_data_line` 入口）。
2. 将 tool 相关实时事件统一从 orchestrator/sink 发射。
