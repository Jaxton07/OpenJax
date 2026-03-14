# 05 Tool Call Streaming And Approval

状态：`in_progress`

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

## 待落地

1. 在工具执行入口发出 `ToolCallArgsDelta`。
2. 对长任务工具输出 `ToolCallProgress`。
3. 出错统一转 `ToolCallFailed`，并保留 `code/message/retryable`。
4. 审批拒绝/超时路径与 `ToolCallFailed` 对齐。

## 验收

1. 一个工具调用能覆盖 started->args_delta*->progress*->completed/failed。
2. 审批拒绝时前端可在同一事件时间线上直观看到中断原因。
