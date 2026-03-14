# 03 Core Streaming Module Design

状态：`done`

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

## 本阶段完成项

1. `chat_completions.rs` 与 `anthropic_messages.rs` 已改为 `SseParser::push_chunk/finish` 主路径。
2. provider 层保留 provider-specific 字段提取；`streaming/parser` 仅负责 SSE 帧归一化。
3. provider 内重复 `pending + line split + data parse` 逻辑已移除。
4. parser/orchestrator/sink/replay 已补齐单测。
