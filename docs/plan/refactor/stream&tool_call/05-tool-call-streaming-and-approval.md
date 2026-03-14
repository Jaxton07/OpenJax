# 05 Tool Call Streaming And Approval

状态：`done`

## 目标语义

1. 工具调用生命周期必须完整可观测。
2. 工具参数支持增量流，不必等待完整参数块。
3. 审批事件与工具事件进入同一时序流。

## 已落地

- `openjax-protocol::Event` 新增：
  - `ToolCallArgsDelta`
  - `ToolCallProgress`
  - `ToolCallFailed`
- `openjax-gateway/openjaxd/ui-tui` 已补齐事件分支消费。

## 本阶段完成项

1. 单工具与批工具执行路径均已补齐 `ToolCallArgsDelta`、`ToolCallProgress`、`ToolCallFailed` 发射点。
2. 审批拒绝/超时在工具失败路径映射为 `ToolCallFailed`（`approval_rejected/approval_timeout`）。
3. 工具 guard 阻断与依赖未满足路径统一进入失败事件模型。
4. 新增 `openjax-core/tests/m21_tool_streaming_events.rs` 校验生命周期顺序与审批拒绝对齐。

## 验收

1. 一个工具调用能覆盖 started->args_delta*->progress*->completed/failed。
2. 审批拒绝时前端可在同一事件时间线上直观看到中断原因。
