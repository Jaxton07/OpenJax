# Gateway Event Mapping

## 映射原则
- 同一 turn 的工具事件优先聚合到同一消息上下文。
- 更新事件应命中已有 step（幂等更新），而非重复新增。

## 事件映射草案
- `tool_call_started` -> 创建/更新 `status=running` 的 ToolStep。
- `tool_call_completed` -> 更新对应 ToolStep 为 `status=success` 并写入 output。
- `approval_requested` -> 创建/更新 `status=waiting` 的 approval step。
- `approval_resolved` -> 将对应 approval step 标记完成或移除等待态。
- `error` -> 创建失败 step 或更新当前 running step 为 failed。

## 需要在实现前确认
- step 主键策略（event id / tool call id / 合成键）。
- approval 与 step 的绑定字段来源。
