# 01 Target Architecture

状态：`done`

## 目标边界

- `openjax-core/src/streaming/` 负责流式事件模型、编排、解析、分发、回放。
- `agent/planner` 只负责业务决策，不负责底层流协议细节。
- `openjax-gateway` 仅负责协议转换与传输，不做复杂业务编排。

## 模块职责

1. `event.rs`
- 定义统一流事件类型与工具生命周期语义。

2. `orchestrator.rs`
- 管理响应生命周期状态机：`started -> delta* -> completed/error`。

3. `parser/`
- 解析 provider SSE 数据块，输出标准 delta 片段。

4. `sink.rs`
- 提供有界队列和背压策略（drop/reject）。

5. `replay.rs`
- 提供会话级回放窗口与越窗错误语义。

## 并发模型

1. 模型流读取和事件发射通过统一 orchestrator 串行化。
2. sink 使用 bounded channel 限制慢消费者内存风险。
3. replay buffer 保持固定容量并返回明确 `min_allowed`。
