# Gateway Event Mapping

## 映射原则
- 同一 turn 的工具事件优先聚合到同一消息上下文。
- 更新事件应命中已有 step（幂等更新），而非重复新增。
- 优先增量更新已有 step 字段，避免覆盖有效历史字段。

## 事件映射（v1）
- `tool_call_started`
  - 主键: `tool_call_id` 或 `turn_id + event_seq`
  - 动作: 创建 step（不存在）或更新 step（已存在）
  - 目标字段: `type=tool`、`status=running`、`title/tool_name`、`subtitle/target`、`time`
- `tool_call_completed`
  - 主键: 同上，命中既有 step
  - 动作: 更新
  - 目标字段: `status=success`、`output`、`time`
- `approval_requested`
  - 主键: `approval_id`
  - 动作: 创建或更新 approval step（仅展示状态）
  - 目标字段: `type=approval`、`status=waiting`、`approvalId`、`toolCallId?`、`description`
- `approval_resolved`
  - 主键: `approval_id`
  - 动作: 更新 approval step 为完成态（`success`），并同步清理审批面板对应项
  - 目标字段: `status=success`、`time`
- `error`
  - 优先命中: 当前 turn 内最近 `running` step
  - 命中时: 更新为 `failed` 并写 `output/message`
  - 未命中时: 创建 `type=summary` 且 `status=failed` 的兜底 step

## 实现前已确认（2026-03-09）
- step 主键策略:
  - 优先使用 `tool_call_id` 作为稳定主键。
  - 若事件缺少 `tool_call_id`，回退为 `turn_id + event_seq` 合成键，保证同 turn 内幂等更新。
- approval 与 step 绑定字段:
  - 审批实体主键使用 `approval_id`。
  - 若 payload 提供 `tool_call_id`，记录该字段用于步骤关联展示；未提供时仅保持审批面板独立流转。

## reducer 处理约束
- 重复事件（`event_seq` 已处理）必须忽略。
- 未知字段不抛错，放入 `meta` 或忽略。
- 未知事件类型不改变既有 step，仅记录日志。
